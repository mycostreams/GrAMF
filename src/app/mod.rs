mod annotate;
mod dev_panel;
mod export_dialog;
#[cfg(target_os = "macos")]
mod native_menu;
mod registration;
mod settings_window;
mod ui;

use crate::annotations::{self, AddedEdge, Annotations, Op};
use crate::colour::vertex_colours;
use annotate::{AnnotationState, Pending, Tool};
use crate::registry;
use crate::renderer::AppRenderer;
use crate::store::Store;
use crate::time::TimeAxis;
use egui_winit::State;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use web_time::Instant;
use winit::application::ApplicationHandler;
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::event::{ElementState, KeyEvent, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::ActiveEventLoop;
use winit::window::{Theme, Window, WindowId};

#[derive(Default)]
pub struct App {
    window: Option<Arc<Window>>,
    renderer: Option<AppRenderer>,
    gui_state: Option<State>,
    last_render_time: Option<Instant>,
    last_size: (u32, u32),
    initialized: bool,
    /// Store to open once the renderer exists (command line or File > Open).
    pending_open: Option<PathBuf>,
    /// One line of feedback in the top bar: what is loaded, or why loading failed.
    status: String,
    cursor: PhysicalPosition<f64>,
    dragging: bool,
    /// Where the left button went down; a release close by is a click, not a drag.
    press: Option<PhysicalPosition<f64>>,
    timeline: Option<ui::Timeline>,
    store: Option<Store>,
    selection: Option<ui::Selection>,
    layers: ui::Layers,
    store_path: Option<PathBuf>,
    annot: Option<AnnotationState>,
    /// Waiting for the user to save or discard unsaved annotations before this happens.
    confirm: Option<Pending>,
    quit: bool,
    /// Last window title set, to avoid resetting it every frame.
    window_title: String,
    export: export_dialog::ExportDialog,
    /// Registry UI of the open plate, else the id found in its path; names export folders.
    plate: String,
    dev: dev_panel::DevState,
    settings: crate::settings::Settings,
    settings_window: settings_window::SettingsWindow,
    workflow: registration::Workflow,
    /// The open plate's video runs (Video registration workflow), when found.
    reg: Option<registration::Registration>,
    /// Routed trails per (author, name): the waypoints and added-edge count they were routed
    /// with, and the node path (`None`: no path).
    trail_routes: HashMap<(String, String), (Vec<i64>, usize, Option<Vec<i64>>)>,
    /// Raw rows for an edge, loaded while the inspector's Raw rows section is open.
    raw: Option<(usize, Vec<crate::devtools::RawTable>)>,
    raw_wanted: bool,
    /// Box select: world corner where the drag started, and the edges of the last box.
    box_start: Option<[f32; 2]>,
    boxed: Vec<usize>,
    /// Nudge tool drag: world point where it started, and the run's nudge before it.
    nudge_drag: Option<([f32; 2], [f32; 2])>,
    /// A file the status line offers to show in Finder / Explorer.
    reveal: Option<PathBuf>,
    /// What the per-edge style table was last built for: (timepoint, colour mode, scale).
    style_key: Option<(Option<usize>, crate::colour::ColourBy, crate::colour::Scale)>,
    reference_width: f32,
    update_check: Option<std::sync::mpsc::Receiver<String>>,
    #[cfg(target_os = "macos")]
    native_menu: Option<native_menu::NativeMenu>,
    update: Option<String>,
}

/// Graph colour while the evidence overlay is on, so the evidence stands out.
const DIM: [f32; 4] = [0.22, 0.22, 0.22, 1.0];
const ARC_SPAN: [f32; 4] = [0.2, 0.9, 1.0, 1.0];

/// Click tolerance (larger for node tools: nodes are small targets) and the most id labels
/// drawn at once.
const PICK_RADIUS_PX: f32 = 6.0;
const NODE_PICK_RADIUS_PX: f32 = 10.0;
/// Node tools draw every node in view up to this many.
const MAX_NODE_DOTS: usize = 6000;
const MAX_LABELS: usize = 1500;

impl App {
    pub fn new(open: Option<PathBuf>) -> Self {
        Self {
            pending_open: open,
            settings: crate::settings::Settings::load(),
            ..Default::default()
        }
    }

    /// The registry record for the store at `path`, a short status for the top bar, and the
    /// plate name (registry UI, else the id found in the path) that names export folders.
    fn resolve_plate(settings: &crate::settings::Settings, path: &Path) -> (Option<registry::PlateRecord>, String, String) {
        let candidates = registry::plate_candidates(path);
        let plate = registry::lookup(&settings.registry(), &candidates);
        let status = match &plate {
            Some(p) => p.ui.clone(),
            None if candidates.is_empty() => "plate id not found in path".into(),
            None => format!("{} not in registry", candidates[0]),
        };
        let name = plate
            .as_ref()
            .map(|p| p.ui.clone())
            .or_else(|| candidates.first().cloned())
            .unwrap_or_else(|| "plate".into());
        (plate, status, name)
    }

    /// Detects the camera orientation on the selected session, and/or saves the session's fit
    /// and camera as the setup default.
    fn camera_and_default(&mut self, detect: bool, save: bool) {
        let (Some(store), Some(reg)) = (self.store.as_ref(), self.reg.as_mut()) else {
            return;
        };
        let i = reg.session;
        let Some(Some(fit)) = reg.fits().into_iter().nth(i) else {
            self.status = "Place the session first (anchor a video)".into();
            return;
        };
        if detect {
            let scores = reg.detect_camera(i, &fit.sim, store);
            let list: Vec<String> = scores.iter().map(|(k, s)| format!("{k}: {s:.1}")).collect();
            let best = scores.first().map_or(reg.camera(), |s| s.0);
            self.status = format!("Camera orientation {best} (fit scores in µm, {})", list.join(", "));
        }
        if save {
            let name = reg.videos.setup.name();
            let default = crate::settings::SetupDefault { transform: fit.sim, camera: reg.camera() };
            self.settings.setup_defaults.insert(name.clone(), default);
            reg.default = Some(fit.sim);
            self.status = match self.settings.save() {
                Ok(()) => format!("Saved this session's placement as the {name} default"),
                Err(e) => format!("Could not save settings: {e}"),
            };
        }
    }

    /// Asks for a `polyflow_analysis` folder and lists its runs.
    fn choose_videos(&mut self) {
        let Some(store) = self.store.as_ref() else {
            return;
        };
        let start = self.store_path.as_ref().and_then(|p| p.parent()).map(Path::to_path_buf);
        let mut dialog = rfd::FileDialog::new().set_title("Folder with the plate's video runs (polyflow_analysis)");
        if let Some(dir) = start {
            dialog = dialog.set_directory(dir);
        }
        let Some(dir) = dialog.pick_folder() else {
            return;
        };
        match crate::video::runs::scan(&dir) {
            Ok(v) if !v.positions.is_empty() => {
                let default = self.settings.setup_default(&v.setup.name());
                self.reg = Some(registration::Registration::new(v, self.settings.session_gap_minutes, store, default));
            }
            Ok(_) => self.status = format!("No video runs in {}", dir.display()),
            Err(e) => self.status = format!("Could not read {}: {e}", dir.display()),
        }
    }

    /// Applies saved settings: author switch, registry re-lookup, persisted to disk.
    fn apply_settings(&mut self, new: crate::settings::Settings) {
        let old = std::mem::replace(&mut self.settings, new);
        if let Some(reg) = self.reg.as_mut() {
            reg.regroup(self.settings.session_gap_minutes);
        }
        if let Err(e) = self.settings.save() {
            self.status = format!("Could not save settings: {e}");
        }
        if let (Some(path), Some(store)) = (self.store_path.clone(), self.store.as_ref()) {
            if old.author != self.settings.author && !self.dirty() {
                let (annot, _) = load_annotations(&path, store, &self.settings.author);
                self.annot = Some(annot);
            }
            if old.registry() != self.settings.registry() {
                let (plate, _, _) = Self::resolve_plate(&self.settings, &path);
                if let Some(t) = self.timeline.as_mut() {
                    t.axis.inoculation = plate.as_ref().and_then(|p| p.inoculation);
                    let first = t.axis.timepoints[0];
                    t.axis.crossing = plate.and_then(|p| p.crossed).map_or((first, true), |c| (c, false));
                }
            }
        }
    }

    fn dirty(&self) -> bool {
        self.annot.as_ref().is_some_and(|a| a.own.dirty)
    }

    fn open_store(&mut self, path: &Path) {
        if self.dirty() {
            self.confirm = Some(Pending::Open(path.to_path_buf()));
            return;
        }
        let Some(renderer) = self.renderer.as_mut() else {
            return;
        };
        self.reveal = None;
        // Dropbox online-only files are 0-byte placeholders that reading does not download.
        if std::fs::metadata(path).is_ok_and(|m| m.len() == 0) {
            self.status = format!(
                "{} is online-only in Dropbox: make it available offline, then open it again",
                path.file_name().map_or(String::new(), |n| n.to_string_lossy().into_owned())
            );
            self.reveal = Some(path.to_path_buf());
            return;
        }
        let started = Instant::now();
        let store = match crate::store::read_store(path) {
            Ok(store) => store,
            Err(e) => {
                self.status = format!("Could not open {}: {e}", path.display());
                return;
            }
        };
        let n_timepoints = store.timepoints.len();
        renderer.load_store(&store, &vertex_colours(&store, self.layers.colour_by, self.layers.scale, n_timepoints));
        renderer.set_evidence(&store, &[]);
        self.selection = None;
        self.dev.reset_store();
        self.raw = None;
        self.style_key = None;
        self.reference_width = crate::colour::reference_width(&store);
        self.dev.evidence_on = false;
        self.dev.timings.load_ms = ms(started);
        let started = Instant::now();

        let (plate, plate_status, plate_name) = Self::resolve_plate(&self.settings, path);
        self.status = format!("{plate_status} · {} edges", store.arcs.n_edges());
        self.plate = plate_name;


        self.timeline = TimeAxis::new(
            store.timepoints.clone(),
            store.quality.clone(),
            plate.as_ref().and_then(|p| p.inoculation),
            plate.as_ref().and_then(|p| p.crossed),
        )
        .map(ui::Timeline::new);
        renderer.time = self.timeline.as_ref().map_or(f32::MAX, |t| t.index as f32);

        let (annot, errors) = load_annotations(path, &store, &self.settings.author);
        if !errors.is_empty() {
            self.status = format!("{} · could not read {}", self.status, errors.join(", "));
        }
        self.annot = Some(annot);
        self.store_path = Some(path.to_path_buf());
        // Video runs beside the store (or one folder up, for <plate>/data/stg_store.h5).
        self.reg = path
            .ancestors()
            .skip(1)
            .take(2)
            .map(|dir| dir.join("polyflow_analysis"))
            .find(|dir| dir.is_dir())
            .and_then(|dir| crate::video::runs::scan(&dir).ok())
            .map(|videos| {
                let default = self.settings.setup_default(&videos.setup.name());
                registration::Registration::new(videos, self.settings.session_gap_minutes, &store, default)
            });
        self.store = Some(store);
        self.dev.timings.annotations_ms = ms(started);
    }

    /// Fills the developer view lazily and keeps the evidence overlay on the current frame.
    fn refresh_dev(&mut self) {
        let (Some(store), Some(path)) = (self.store.as_ref(), self.store_path.as_ref()) else {
            return;
        };
        let d = &mut self.dev;
        if (d.open || self.raw_wanted) && d.datasets.is_none() {
            d.datasets = Some(crate::devtools::inspect(path));
        }
        if let (true, true, Some(Ok(list))) = (d.open, d.health.is_empty(), &d.datasets) {
            let annotations: Vec<&Annotations> = self
                .annot
                .iter()
                .flat_map(|st| std::iter::once(&st.own).chain(st.others.iter().map(|(a, _)| a)))
                .collect();
            d.health = crate::devtools::health(path, store, list, &annotations);
        }
        if let (true, Some(ui::Selection::Edge(e)), Some(Ok(list))) = (self.raw_wanted, self.selection, &d.datasets) {
            if self.raw.as_ref().map(|r| r.0) != Some(e) {
                self.raw = Some((e, crate::devtools::raw_rows(path, list, store.n_edges(), e)));
            }
        }
        let (Some(renderer), Some(timeline)) = (self.renderer.as_mut(), self.timeline.as_ref()) else {
            return;
        };
        if d.evidence_on && d.evidence_frame != Some(timeline.index) {
            let started = Instant::now();
            let evidence = d
                .evidence
                .get_or_insert_with(|| crate::devtools::read_evidence(path, timeline.axis.len(), store.n_edges()));
            if let Ok(ev) = evidence {
                let spans: Vec<(usize, f32, f32, [f32; 4])> = ev.by_frame[timeline.index]
                    .iter()
                    .map(|&(e, a, b)| {
                        if ev.arc_spans {
                            (e, a, b, ARC_SPAN)
                        } else {
                            let quality = if ev.score_semantic == "coverage" { a } else { 1.0 - a / ev.threshold };
                            (e, f32::NEG_INFINITY, f32::INFINITY, crate::colour::Cmap::Viridis.sample(quality))
                        }
                    })
                    .collect();
                renderer.set_evidence(store, &spans);
            }
            d.evidence_frame = Some(timeline.index);
            d.timings.evidence_ms = ms(started);
        }
    }

    fn save_annotations(&mut self) -> bool {
        self.refresh_trail_routes();
        // With video runs loaded, the flow layer is recomputed from the verified sessions;
        // otherwise the one last loaded is kept.
        let mut flow_note = String::new();
        if let (Some(reg), Some(store), Some(st)) = (self.reg.as_mut(), self.store.as_ref(), self.annot.as_mut()) {
            let added = added_segments(store, Some(&*st));
            let (flow, sessions, runs) = registration::flow_output(reg, store, &added);
            flow_note = format!(" · flow: {sessions} verified sessions, {runs} runs, {} measurements", flow.measurements.len());
            st.own.flow = (!flow.runs.is_empty()).then_some(flow);
        }
        let trail_paths = self.trail_paths();
        let Some(st) = self.annot.as_mut() else {
            return false;
        };
        match annotations::save(&st.own, &st.own_path, &trail_paths) {
            Ok(()) => {
                st.own.dirty = false;
                self.status = format!("Saved {}{flow_note}", st.own_path.display());
                true
            }
            Err(e) => {
                self.status = format!("Could not save {}: {e}", st.own_path.display());
                false
            }
        }
    }

    /// The active author's trails' routed node paths.
    fn trail_paths(&self) -> std::collections::BTreeMap<String, Vec<i64>> {
        let Some(st) = self.annot.as_ref() else {
            return Default::default();
        };
        st.own
            .trails
            .keys()
            .filter_map(|name| {
                let path = self.trail_routes.get(&(st.author.clone(), name.clone()))?.2.clone()?;
                Some((name.clone(), path))
            })
            .collect()
    }

    /// Publish to store (DESIGN.md §10): saves, asks, then writes the active author's flow
    /// layer, trails and roots into the store.
    fn publish_to_store(&mut self) {
        if !self.save_annotations() {
            return;
        }
        let trail_paths = self.trail_paths();
        let (Some(store), Some(path), Some(st)) = (self.store.as_mut(), self.store_path.as_ref(), self.annot.as_ref()) else {
            return;
        };
        let Some(flow) = st.own.flow.as_ref() else {
            self.status = "Nothing to publish: verify a session first".into();
            return;
        };
        let question = format!(
            "Write {}'s flow layer ({} runs, {} measurements), {} trails and {} roots into {}?\n\n\
             This replaces the store's /flow_runs, /flow_measurements, /flow_trails_* and /root_nodes. \
             Nothing else in the store changes.",
            st.author,
            flow.runs.len(),
            flow.measurements.len(),
            st.own.trails.len(),
            st.own.roots.len(),
            path.display()
        );
        let ok = rfd::MessageDialog::new()
            .set_title("Publish to store")
            .set_description(question)
            .set_buttons(rfd::MessageButtons::OkCancel)
            .show();
        if ok != rfd::MessageDialogResult::Ok {
            return;
        }
        match annotations::publish(&st.own, path, store.n_edges(), &trail_paths) {
            Ok(p) => {
                // what the store now holds, without re-reading it
                store.root_nodes = st.own.roots.iter().copied().collect();
                store.trails_in_store = trail_paths.into_iter().collect();
                store.flow_in_store = Some(crate::store::FlowLayer {
                    run_paths: flow.runs.iter().map(|r| r.run_path.to_string()).collect(),
                    n_measurements: p.measurements,
                    written_by: "GrAMF".into(),
                });
                let skipped = match p.on_added_edges {
                    0 => String::new(),
                    n => format!(" ({n} on added edges stay in the annotation file)"),
                };
                self.status = format!(
                    "Published to {}: {} measurements{skipped}, {} trails, {} roots",
                    path.display(),
                    p.measurements,
                    p.trails,
                    p.roots
                );
            }
            Err(e) => self.status = format!("Could not publish to {}: {e}", path.display()),
        }
    }

    fn export_tables(&mut self) {
        let (Some(store), Some(path), Some(renderer)) = (self.store.as_ref(), self.store_path.as_ref(), self.renderer.as_ref())
        else {
            return;
        };
        let d = &self.export;
        let (edges, scope): (Vec<usize>, String) = match d.scope {
            export_dialog::Scope::Plate => ((0..store.n_edges()).collect(), "whole plate".into()),
            export_dialog::Scope::View => {
                let (min, max) = renderer.visible_world();
                let inside = |p: &[f32; 2]| p[0] >= min[0] && p[0] <= max[0] && p[1] >= min[1] && p[1] <= max[1];
                let t = renderer.time;
                let edges = (0..store.n_edges())
                    .filter(|&e| store.edge_t[e] <= t)
                    .filter(|&e| store.arcs.points[store.arcs.offsets[e]..store.arcs.offsets[e + 1]].iter().any(inside))
                    .collect();
                let t_label = if t.is_finite() && t < f32::MAX { format!("timepoint {t}") } else { "all time".into() };
                (edges, format!("in view x {:.0}..{:.0} µm, y {:.0}..{:.0} µm, {t_label}", min[0], max[0], min[1], max[1]))
            }
            export_dialog::Scope::Selection => match self.selection {
                Some(ui::Selection::Edge(e)) => (vec![e], format!("edge {e}")),
                Some(ui::Selection::Node(n)) => (store.incident_edges(n), format!("edges of node {}", store.node_ids[n])),
                Some(ui::Selection::Hyperedge(h)) => (store.hyperedge_edges(h), format!("hyperedge {h}")),
                Some(ui::Selection::Added(id)) => (vec![], format!("added edge {id}")),
                Some(ui::Selection::Box) => (self.boxed.clone(), format!("box selection of {} edges", self.boxed.len())),
                None => (vec![], "empty selection".into()),
            },
        };
        let annotations = self.annot.as_ref().and_then(|st| {
            let author = d.author.as_ref()?;
            if *author == st.author {
                Some(&st.own)
            } else {
                st.others.iter().map(|(a, _)| a).find(|a| a.author == *author)
            }
        });
        let Some(folder) = rfd::FileDialog::new().set_title("Export tables into…").pick_folder() else {
            return;
        };
        let req = crate::export::Request {
            store,
            store_path: path,
            plate: &self.plate,
            axis: self.timeline.as_ref().map(|t| &t.axis),
            annotations,
            edges,
            scope,
            keep_excluded: !d.drop_flagged,
            tables: d.tables.clone(),
        };
        let started = Instant::now();
        self.status = match crate::export::export(&folder, &req) {
            Ok(dir) => format!("Exported to {}", dir.display()),
            Err(e) => format!("Export failed: {e}"),
        };
        self.dev.timings.export_ms = ms(started);
    }

    /// Re-routes trails whose waypoints or added edges changed since they were last routed.
    fn refresh_trail_routes(&mut self) {
        let (Some(store), Some(st)) = (self.store.as_ref(), self.annot.as_ref()) else {
            return;
        };
        let layers = std::iter::once(&st.own).chain(st.others.iter().map(|(a, _)| a));
        let mut live = std::collections::HashSet::new();
        for a in layers {
            let pos = |id: &i64| store.node_index.get(id).map(|&n| store.node_pos[n]);
            for (name, waypoints) in &a.trails {
                let key = (a.author.clone(), name.clone());
                live.insert(key.clone());
                let fresh = self.trail_routes.get(&key).is_some_and(|(w, n, _)| w == waypoints && *n == a.added.len());
                if !fresh {
                    let extra: Vec<(i64, i64, f32)> = a
                        .added
                        .iter()
                        .filter_map(|e| {
                            let (p, q) = (pos(&e.u)?, pos(&e.v)?);
                            Some((e.u, e.v, ((q[0] - p[0]).powi(2) + (q[1] - p[1]).powi(2)).sqrt()))
                        })
                        .collect();
                    let route = store.route(waypoints, &extra);
                    self.trail_routes.insert(key, (waypoints.clone(), a.added.len(), route));
                }
            }
        }
        self.trail_routes.retain(|k, _| live.contains(k));
    }

    /// Quits, or first asks what to do with unsaved annotations.
    fn request_quit(&mut self) {
        if self.dirty() {
            self.confirm = Some(Pending::Quit);
        } else {
            self.quit = true;
        }
    }

    /// Acts on the macOS menu bar.
    #[cfg(target_os = "macos")]
    fn handle_native_menu(&mut self) {
        use native_menu::Command;
        for command in native_menu::NativeMenu::poll() {
            let mut actions = ui::UiActions::default();
            match command {
                Command::Open => actions.open = true,
                Command::Save => actions.save = true,
                Command::Export => self.export.open = self.store.is_some(),
                Command::Undo => actions.undo = true,
                Command::Redo => actions.redo = true,
                Command::ResetCamera => actions.fit = true,
                Command::Developer => self.dev.open = !self.dev.open,
                Command::Quit => self.request_quit(),
                Command::Settings => actions.open_settings = true,
            }
            self.apply(actions);
        }
        let (store_open, developer) = (self.store.is_some(), self.dev.open);
        if let Some(menu) = self.native_menu.as_mut() {
            menu.sync(store_open, developer);
        }
    }

    /// Carries out what waited on the unsaved-annotations prompt.
    fn proceed(&mut self, pending: Pending) {
        if let Some(st) = self.annot.as_mut() {
            st.own.dirty = false; // saved or discarded by now
        }
        match pending {
            Pending::Quit => self.quit = true,
            Pending::Open(path) => self.pending_open = Some(path),
        }
    }

    fn select(&mut self, selection: Option<ui::Selection>) {
        let (Some(renderer), Some(store)) = (self.renderer.as_mut(), self.store.as_ref()) else {
            return;
        };
        self.selection = selection;
        let edges = match selection {
            Some(ui::Selection::Edge(e)) => vec![e],
            Some(ui::Selection::Node(n)) => store.incident_edges(n),
            Some(ui::Selection::Hyperedge(h)) => store.hyperedge_edges(h),
            Some(ui::Selection::Box) => self.boxed.clone(),
            Some(ui::Selection::Added(_)) | None => vec![], // the overlay highlights added edges
        };
        renderer.set_selection(store, &edges);
    }

    /// Acts on a click at window pixel `px` with the current tool.
    fn pick(&mut self, px: [f32; 2]) {
        let started = Instant::now();
        if self.workflow == registration::Workflow::Registration {
            self.registration_click(px);
        } else {
            self.pick_inner(px);
        }
        self.dev.timings.pick_ms = ms(started);
    }

    /// A map click in the registration workspace: select a video, or anchor the selected one.
    fn registration_click(&mut self, px: [f32; 2]) {
        let (Some(renderer), Some(store), Some(reg)) = (self.renderer.as_ref(), self.store.as_ref(), self.reg.as_mut()) else {
            return;
        };
        let at = renderer.screen_to_world(px);
        let added = added_segments(store, self.annot.as_ref());
        let radius = PICK_RADIUS_PX * renderer.um_per_px();
        match (reg.tool, reg.position) {
            (registration::RegTool::Assign, Some(_)) => {
                let edge = store
                    .pick_edge(at, radius, f32::MAX)
                    .map(crate::video::matching::EdgeRef::Pipeline)
                    .or_else(|| {
                        added
                            .iter()
                            .map(|a| (crate::store::segment_dist2(at, a.a, a.b).sqrt(), a.id))
                            .filter(|x| x.0 <= radius)
                            .min_by(|a, b| a.0.total_cmp(&b.0))
                            .map(|x| crate::video::matching::EdgeRef::Added(x.1))
                    });
                if let Some(op) = edge.and_then(|e| registration::toggle_in_chain(reg, store, &added, e)) {
                    reg.pending.push(op);
                }
            }
            (registration::RegTool::Disable, Some(p)) => {
                if let Some(key) = registration::video_edge_at(reg, store, &added, at, radius) {
                    let run_edge = (reg.run_path(p, reg.shown_run(p)), key);
                    let now = reg.layer.disabled.contains(&run_edge);
                    reg.pending.push(Op::SetDisabled(run_edge, !now));
                    reg.video_edge = Some(key);
                }
            }
            (registration::RegTool::Nudge, _) => (), // drags, not clicks
            (registration::RegTool::Anchor, Some(p)) => {
                let snap = store.pick_node(at, NODE_PICK_RADIUS_PX * renderer.um_per_px(), renderer.time);
                let w = snap.map_or(at, |n| store.node_pos[n]);
                let name = reg.videos.positions[p].name.clone();
                reg.pending.push(Op::SetAnchor(name, Some([w[0] as f64, w[1] as f64])));
            }
            _ => registration::pick(reg, store, &added, at, radius),
        }
    }

    fn pick_inner(&mut self, px: [f32; 2]) {
        let (Some(renderer), Some(store), Some(st)) = (self.renderer.as_ref(), self.store.as_ref(), self.annot.as_mut())
        else {
            return;
        };
        let at = renderer.screen_to_world(px);
        let radius = PICK_RADIUS_PX * renderer.um_per_px();
        let node_radius = NODE_PICK_RADIUS_PX * renderer.um_per_px();
        let t = renderer.time;
        let node = || store.pick_node(at, if st.tool.needs_nodes() { node_radius } else { radius }, t);
        let edge = || store.pick_edge(at, radius, t);

        let mut op = None;
        let selection = match st.tool {
            Tool::BoxSelect => return, // boxes are drawn by dragging; a plain click does nothing
            Tool::Select => node()
                .map(ui::Selection::Node)
                .or_else(|| pick_added(store, st, at, radius).map(ui::Selection::Added))
                .or_else(|| edge().map(ui::Selection::Edge)),
            Tool::Exclude => edge().map(|e| {
                op = Some(Op::SetExcluded(e, !st.own.excluded.contains(&e)));
                ui::Selection::Edge(e)
            }),
            Tool::Root => node().map(|n| {
                let id = store.node_ids[n];
                op = Some(Op::SetRoot(id, !st.own.roots.contains(&id)));
                ui::Selection::Node(n)
            }),
            Tool::Merge => edge().map(|e| match st.first.take() {
                Some(dup) if dup != e => {
                    op = Some(Op::SetDuplicateOf(dup, Some(e)));
                    ui::Selection::Edge(dup)
                }
                _ => {
                    st.first = Some(e);
                    ui::Selection::Edge(e)
                }
            }),
            Tool::Trail => node().map(|n| {
                let id = store.node_ids[n];
                let name = st.active_trail.clone().filter(|t| st.own.trails.contains_key(t)).unwrap_or_else(|| {
                    (1..).map(|k| format!("trail {k}")).find(|t| !st.own.trails.contains_key(t)).expect("unbounded")
                });
                let mut waypoints = st.own.trails.get(&name).cloned().unwrap_or_default();
                if waypoints.last() != Some(&id) {
                    waypoints.push(id);
                    op = Some(Op::SetTrail(name.clone(), Some(waypoints)));
                }
                st.active_trail = Some(name);
                ui::Selection::Node(n)
            }),
            Tool::AddEdge => node().map(|n| match st.first.take() {
                Some(first) if first != n => {
                    let added = AddedEdge {
                        id: st.own.next_added_id(),
                        u: store.node_ids[first],
                        v: store.node_ids[n],
                    };
                    op = Some(Op::AddEdge(added));
                    ui::Selection::Added(added.id)
                }
                _ => {
                    st.first = Some(n);
                    ui::Selection::Node(n)
                }
            }),
        };
        if let Some(op) = op {
            st.own.edit(op);
        }
        if selection.is_some() || st.tool == Tool::Select {
            self.select(selection);
        }
    }

    /// Selects the edges with a vertex inside the box `a`–`b` (µm) that have appeared by now.
    fn box_select(&mut self, a: [f32; 2], b: [f32; 2]) {
        let (Some(store), Some(renderer)) = (self.store.as_ref(), self.renderer.as_ref()) else {
            return;
        };
        let (min, max) = ([a[0].min(b[0]), a[1].min(b[1])], [a[0].max(b[0]), a[1].max(b[1])]);
        let t = renderer.time;
        let (p, vt) = (&store.arcs.points, &store.vertex_t);
        self.boxed = (0..store.n_edges())
            .filter(|&e| {
                (store.arcs.offsets[e]..store.arcs.offsets[e + 1]).any(|v| {
                    vt[v] <= t && p[v][0] >= min[0] && p[v][0] <= max[0] && p[v][1] >= min[1] && p[v][1] <= max[1]
                })
            })
            .collect();
        let selection = (!self.boxed.is_empty()).then_some(ui::Selection::Box);
        self.select(selection);
    }

    fn apply(&mut self, actions: ui::UiActions) {
        if actions.colour_changed || actions.evidence_toggled {
            if let (Some(renderer), Some(store)) = (self.renderer.as_mut(), self.store.as_ref()) {
                let colours = if self.dev.evidence_on {
                    vec![DIM; store.arcs.points.len()]
                } else {
                    vertex_colours(store, self.layers.colour_by, self.layers.scale, store.timepoints.len())
                };
                renderer.set_colours(store, &colours);
                if !self.dev.evidence_on {
                    renderer.set_evidence(store, &[]);
                }
                self.dev.evidence_frame = None;
                self.select(self.selection); // set_colours drops the highlight
            }
        }
        if actions.select.is_some() {
            self.select(actions.select);
        }
        if let Some(st) = self.annot.as_mut() {
            for op in actions.ops {
                st.own.edit(op);
            }
            if actions.undo {
                st.own.undo();
            }
            if actions.redo {
                st.own.redo();
            }
        }
        if actions.save {
            self.save_annotations();
        }
        if actions.open_settings {
            self.settings_window.open(&self.settings);
        }
        if let Some(new) = actions.settings_saved {
            self.apply_settings(new);
        }
        if let Some(choice) = actions.choice {
            if let Some(pending) = self.confirm.take() {
                match choice {
                    annotate::Choice::Save if self.save_annotations() => self.proceed(pending),
                    annotate::Choice::Save => self.confirm = Some(pending), // save failed: ask again
                    annotate::Choice::Discard => self.proceed(pending),
                    annotate::Choice::Cancel => (),
                }
            }
        }
        if actions.export {
            self.export_tables();
        }
        if actions.choose_videos {
            self.choose_videos();
        }
        if actions.publish {
            self.publish_to_store();
        }
        if actions.detect_camera || actions.save_setup_default {
            self.camera_and_default(actions.detect_camera, actions.save_setup_default);
        }
        self.raw_wanted = actions.raw_wanted;
        if let (true, Some(path)) = (actions.reveal, self.reveal.as_ref()) {
            reveal_in_file_manager(path);
        }
        if actions.fit {
            if let Some(renderer) = self.renderer.as_mut() {
                renderer.fit();
            }
        }
        if actions.open {
            self.pending_open = rfd::FileDialog::new()
                .add_filter("STG store", &["h5"])
                .pick_file();
        }
    }
}

/// Shows `path` selected in Finder / Explorer (its folder elsewhere).
fn reveal_in_file_manager(path: &Path) {
    let result = if cfg!(target_os = "macos") {
        std::process::Command::new("open").arg("-R").arg(path).spawn()
    } else if cfg!(windows) {
        std::process::Command::new("explorer").arg(format!("/select,{}", path.display())).spawn()
    } else {
        std::process::Command::new("xdg-open").arg(path.parent().unwrap_or(path)).spawn()
    };
    if let Err(e) = result {
        log::warn!("could not reveal {}: {e}", path.display());
    }
}

fn ms(since: Instant) -> f64 {
    since.elapsed().as_secs_f64() * 1000.0
}

/// The user's own added edges as straight segments (matching includes them).
fn added_segments(store: &Store, annot: Option<&AnnotationState>) -> Vec<crate::video::matching::AddedSegment> {
    let pos = |id: &i64| store.node_index.get(id).map(|&n| store.node_pos[n]);
    annot
        .into_iter()
        .flat_map(|st| &st.own.added)
        .filter_map(|e| Some(crate::video::matching::AddedSegment { id: e.id, a: pos(&e.u)?, b: pos(&e.v)? }))
        .collect()
}

/// The user's own annotation file for the store at `path` (new if absent) and everyone
/// else's beside it; returns the files that could not be read.
fn load_annotations(path: &Path, store: &Store, author: &str) -> (AnnotationState, Vec<String>) {
    let mut errors = vec![];
    let own_path = annotations::path_for(path, author);
    let own = if own_path.exists() {
        annotations::load(&own_path).unwrap_or_else(|e| {
            errors.push(e);
            Annotations::new(author, &store.fingerprint)
        })
    } else {
        Annotations::new(author, &store.fingerprint)
    };
    let others = annotations::list(path)
        .into_iter()
        .filter(|(a, _)| a != author)
        .filter_map(|(_, p)| annotations::load(&p).map_err(|e| errors.push(e)).ok())
        .collect();
    (AnnotationState::new(author, own, own_path, others), errors)
}

/// Nearest added edge (own or a visible other author's) within `radius` µm.
fn pick_added(store: &Store, st: &AnnotationState, at: [f32; 2], radius: f32) -> Option<i64> {
    let pos = |id: &i64| store.node_index.get(id).map(|&n| store.node_pos[n]);
    std::iter::once((&st.own, st.own_visible))
        .chain(st.others.iter().map(|(a, v)| (a, *v)))
        .filter(|(_, visible)| *visible)
        .flat_map(|(a, _)| &a.added)
        .filter_map(|e| Some((e.id, crate::store::segment_dist2(at, pos(&e.u)?, pos(&e.v)?))))
        .filter(|&(_, d)| d <= radius * radius)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(id, _)| id)
}

const TRAIL: egui::Color32 = egui::Color32::from_rgb(235, 90, 210);

/// Every visible author's trails, in points: the routed path (straight dashed legs when no
/// path exists) and waypoints as shapes, plus each trail's name at its first waypoint as a
/// map text. The active trail is thicker.
fn trail_overlay(
    store: &Store,
    st: &AnnotationState,
    routes: &HashMap<(String, String), (Vec<i64>, usize, Option<Vec<i64>>)>,
    renderer: &AppRenderer,
    ppp: f32,
) -> (Vec<egui::Shape>, Vec<(egui::Pos2, String, egui::Color32)>) {
    use egui::{Color32, Shape, Stroke};
    let to_pt = |p: [f32; 2]| {
        let [x, y] = renderer.world_to_screen(p);
        egui::pos2(x / ppp, y / ppp)
    };
    let pos = |id: &i64| store.node_index.get(id).map(|&n| store.node_pos[n]);
    let (mut shapes, mut texts) = (vec![], vec![]);
    let layers = std::iter::once((&st.own, st.own_visible, true)).chain(st.others.iter().map(|(a, v)| (a, *v, false)));
    for (a, _, own) in layers.filter(|l| l.1) {
        let colour = if own { TRAIL } else { Color32::from_rgba_unmultiplied(235, 90, 210, 110) };
        for (name, waypoints) in &a.trails {
            let active = own && st.active_trail.as_deref() == Some(name.as_str());
            let stroke = Stroke::new(if active { 4.0 } else if own { 3.0 } else { 2.0 }, colour);
            match routes.get(&(a.author.clone(), name.clone())).and_then(|r| r.2.as_ref()) {
                Some(path) => shapes.push(Shape::line(store.path_polyline(path).into_iter().map(to_pt).collect(), stroke)),
                None => {
                    let points: Vec<egui::Pos2> = waypoints.iter().filter_map(pos).map(to_pt).collect();
                    shapes.extend(Shape::dashed_line(&points, stroke, 5.0, 5.0));
                }
            }
            for p in waypoints.iter().filter_map(pos) {
                shapes.push(Shape::circle_filled(to_pt(p), if active { 5.0 } else { 3.5 }, colour));
            }
            if let Some(first) = waypoints.first().and_then(pos) {
                texts.push((to_pt(first) + egui::vec2(8.0, -10.0), name.clone(), colour));
            }
        }
    }
    (shapes, texts)
}

/// A thin dashed line from each arc end that stops short of its node to that node, for edges
/// that have appeared, clipped to the view so a long gap seen up close stays a few dashes.
fn detached_overlay(store: &Store, renderer: &AppRenderer, to_pt: impl Fn([f32; 2]) -> egui::Pos2) -> Vec<egui::Shape> {
    let (min, max) = renderer.visible_world();
    let stroke = egui::Stroke::new(1.0, egui::Color32::from_rgb(240, 170, 60));
    let mut shapes = vec![];
    for d in store.detached_ends() {
        if store.edge_t[d.edge] > renderer.time {
            continue;
        }
        if let Some((p, q)) = clip_segment(store.arcs.points[d.point], store.node_pos[d.node], min, max) {
            shapes.extend(egui::Shape::dashed_line(&[to_pt(p), to_pt(q)], stroke, 4.0, 3.0));
        }
    }
    shapes
}

/// The part of segment `p`-`q` inside the box `min`..`max` (Liang-Barsky), if any.
fn clip_segment(p: [f32; 2], q: [f32; 2], min: [f32; 2], max: [f32; 2]) -> Option<([f32; 2], [f32; 2])> {
    let (mut t0, mut t1) = (0.0f32, 1.0f32);
    for k in 0..2 {
        let d = q[k] - p[k];
        for (num, den) in [(p[k] - min[k], -d), (max[k] - p[k], d)] {
            if den == 0.0 {
                if num < 0.0 {
                    return None;
                }
            } else {
                let t = num / den;
                if den < 0.0 { t0 = t0.max(t) } else { t1 = t1.min(t) }
            }
        }
    }
    let at = |t: f32| [p[0] + (q[0] - p[0]) * t, p[1] + (q[1] - p[1]) * t];
    (t0 <= t1).then(|| (at(t0), at(t1)))
}

/// Feedback for the current tool, in points: node dots and the node under the cursor for node
/// tools, a rubber band from the first node while adding an edge, and the box being dragged.
fn tool_overlay(
    store: &Store,
    st: &AnnotationState,
    renderer: &AppRenderer,
    ppp: f32,
    cursor: [f32; 2],
    box_start: Option<[f32; 2]>,
) -> Vec<egui::Shape> {
    use egui::{Color32, Shape, Stroke};
    let to_pt = |p: [f32; 2]| {
        let [x, y] = renderer.world_to_screen(p);
        egui::pos2(x / ppp, y / ppp)
    };
    let cursor_pt = egui::pos2(cursor[0] / ppp, cursor[1] / ppp);
    let mut shapes = vec![];
    let t = renderer.time;

    if st.tool.needs_nodes() {
        let (min, max) = renderer.visible_world();
        let visible: Vec<usize> = (0..store.node_pos.len())
            .filter(|&n| store.node_t[n] <= t)
            .filter(|&n| {
                let p = store.node_pos[n];
                p[0] >= min[0] && p[0] <= max[0] && p[1] >= min[1] && p[1] <= max[1]
            })
            .take(MAX_NODE_DOTS + 1)
            .collect();
        if visible.len() <= MAX_NODE_DOTS {
            let dot = Color32::from_rgba_unmultiplied(220, 220, 220, 160);
            shapes.extend(visible.iter().map(|&n| Shape::circle_filled(to_pt(store.node_pos[n]), 2.0, dot)));
        }
        let at = renderer.screen_to_world(cursor);
        let hover = store.pick_node(at, NODE_PICK_RADIUS_PX * renderer.um_per_px(), t);
        let accent = Color32::from_rgb(60, 210, 230);
        if let Some(n) = hover {
            shapes.push(Shape::circle_stroke(to_pt(store.node_pos[n]), 7.0, Stroke::new(2.0, accent)));
        }
        let trail_end = (st.tool == Tool::Trail)
            .then(|| st.active_trail.as_ref().and_then(|t| st.own.trails.get(t)?.last()))
            .flatten()
            .and_then(|id| store.node_index.get(id).copied());
        if let Some(last) = trail_end {
            let from = to_pt(store.node_pos[last]);
            let to = hover.map_or(cursor_pt, |n| to_pt(store.node_pos[n]));
            shapes.extend(Shape::dashed_line(&[from, to], Stroke::new(2.0, TRAIL), 6.0, 4.0));
        }
        if let (Tool::AddEdge, Some(first)) = (st.tool, st.first) {
            let from = to_pt(store.node_pos[first]);
            let to = hover.map_or(cursor_pt, |n| to_pt(store.node_pos[n]));
            shapes.push(Shape::circle_filled(from, 4.0, accent));
            shapes.extend(Shape::dashed_line(&[from, to], Stroke::new(2.0, accent), 6.0, 4.0));
        }
    }
    if let (Tool::BoxSelect, Some(start)) = (st.tool, box_start) {
        let rect = egui::Rect::from_two_pos(to_pt(start), cursor_pt);
        shapes.push(Shape::rect_filled(rect, 0.0, Color32::from_rgba_unmultiplied(60, 210, 230, 30)));
        shapes.push(Shape::rect_stroke(rect, 0.0, Stroke::new(1.5, Color32::from_rgb(60, 210, 230)), egui::StrokeKind::Inside));
    }
    shapes
}

/// Id labels inside the view for items that have appeared by the current time, in points;
/// `None` when more than [`MAX_LABELS`] would be drawn.
fn id_labels(store: &Store, layers: &ui::Layers, renderer: &AppRenderer, ppp: f32) -> Option<Vec<(egui::Pos2, String)>> {
    let (min, max) = renderer.visible_world();
    let inside = |p: [f32; 2]| p[0] >= min[0] && p[0] <= max[0] && p[1] >= min[1] && p[1] <= max[1];
    let t = renderer.time;
    let mut items: Vec<([f32; 2], String)> = Vec::new();
    if layers.node_ids {
        for n in (0..store.node_pos.len()).filter(|&n| store.node_t[n] <= t && inside(store.node_pos[n])) {
            items.push((store.node_pos[n], store.node_ids[n].to_string()));
            if items.len() > MAX_LABELS {
                return None;
            }
        }
    }
    if layers.hyperedge_ids {
        for (h, p, ht) in &store.hyperedge_anchors {
            if *ht <= t && inside(*p) {
                items.push((*p, format!("H{h}")));
                if items.len() > MAX_LABELS {
                    return None;
                }
            }
        }
    }
    Some(
        items
            .into_iter()
            .map(|(p, text)| {
                let [x, y] = renderer.world_to_screen(p);
                (egui::pos2(x / ppp, y / ppp), text)
            })
            .collect(),
    )
}

impl ApplicationHandler for App {
    /// Called when the application is suspended (e.g., minimized or sent to the background).
    fn suspended(&mut self, _event_loop: &winit::event_loop::ActiveEventLoop) {
        self.renderer = None;
        self.window = None;
    }

    /// Called when the application is resumed (e.g., restored or brought to the foreground).
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        let mut attributes = Window::default_attributes();

        attributes = attributes.with_title("GrAMF");

        let Ok(window) = event_loop.create_window(attributes) else {
            return;
        };

        let window_handle = Arc::new(window);
        self.window = Some(window_handle.clone());

        let gui_context = egui::Context::default();

        let inner_size = window_handle.inner_size();
        self.last_size = (inner_size.width, inner_size.height);

        let viewport_id = gui_context.viewport_id();
        let gui_state = State::new(
            gui_context,
            viewport_id,
            &window_handle,
            Some(window_handle.scale_factor() as _),
            Some(Theme::Dark),
            None,
        );

        let (width, height) = (
            window_handle.inner_size().width,
            window_handle.inner_size().height,
        );

        if !self.initialized {
            env_logger::init();
            self.update_check = Some(crate::update::spawn_check());
            #[cfg(target_os = "macos")]
            {
                self.native_menu = native_menu::NativeMenu::install();
            }
        }
        let renderer =
            pollster::block_on(
                async move { AppRenderer::new(window_handle.clone(), width, height).await },
            );
        self.renderer = Some(renderer);

        self.gui_state = Some(gui_state);
        self.last_render_time = Some(Instant::now());
        self.initialized = true;

        if let Some(path) = self.pending_open.take() {
            self.open_store(&path);
        }
    }

    /// Called when a window event occurs (e.g., keyboard input, resizing, etc.).
    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        // Ensure that the GUI state, renderer, window, and last render time are all available before processing the event
        let (Some(gui_state), Some(renderer), Some(window), Some(last_render_time)) = (
            self.gui_state.as_mut(),
            self.renderer.as_mut(),
            self.window.as_ref(),
            self.last_render_time.as_mut(),
        ) else {
            return;
        };

        // Pass the event to the GUI state for handling, and if it was consumed, return early
        if gui_state.on_window_event(window, &event).consumed {
            return;
        }

        // Acted on after the event, once the borrows above are released
        let mut ui_actions = None;
        let mut click = None;
        let mut boxed = None;
        let mut escape = false;
        let mut close_requested = false;

        // Handle specific window events such as keyboard input, resizing, and redraw requests
        match event {
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        physical_key: winit::keyboard::PhysicalKey::Code(key_code),
                        state,
                        ..
                    },
                ..
            } => match (key_code, state) {
                (winit::keyboard::KeyCode::Escape, ElementState::Pressed) => escape = true,
                (winit::keyboard::KeyCode::Home, ElementState::Pressed) => renderer.fit(),
                (key, ElementState::Pressed) => {
                    if let Some(timeline) = self.timeline.as_mut() {
                        match key {
                            winit::keyboard::KeyCode::ArrowLeft => timeline.step(-1),
                            winit::keyboard::KeyCode::ArrowRight => timeline.step(1),
                            winit::keyboard::KeyCode::Space => timeline.toggle_play(),
                            _ => (),
                        }
                    }
                }
                _ => (),
            },
            WindowEvent::MouseInput {
                state,
                button: MouseButton::Left,
                ..
            } => {
                let registering = self.workflow == registration::Workflow::Registration;
                let boxing = !registering && self.annot.as_ref().is_some_and(|a| a.tool == Tool::BoxSelect);
                let nudging = registering
                    && self.reg.as_ref().is_some_and(|r| r.tool == registration::RegTool::Nudge && r.position.is_some());
                let cursor = [self.cursor.x as f32, self.cursor.y as f32];
                let pressed = state == ElementState::Pressed;
                if let (true, Some(reg)) = (nudging, self.reg.as_mut()) {
                    let p = reg.position.unwrap_or(0);
                    let rel = reg.run_path(p, reg.shown_run(p));
                    if pressed {
                        let base = reg.layer.nudges.get(&rel).copied().unwrap_or([0.0; 2]);
                        self.nudge_drag = Some((renderer.screen_to_world(cursor), base));
                    } else if self.nudge_drag.take().is_some() {
                        if let Some(n) = reg.drag_nudge.take() {
                            reg.pending.push(Op::SetNudge(rel, Some(n)));
                        }
                    }
                } else if boxing {
                    if pressed {
                        self.box_start = Some(renderer.screen_to_world(cursor));
                    } else if let Some(start) = self.box_start.take() {
                        boxed = Some((start, renderer.screen_to_world(cursor)));
                    }
                } else {
                    self.dragging = pressed;
                    if pressed {
                        self.press = Some(self.cursor);
                    } else if let Some(p) = self.press.take() {
                        if (p.x - self.cursor.x).hypot(p.y - self.cursor.y) < 4.0 {
                            click = Some(cursor);
                        }
                    }
                }
            }
            // Right-drag pans with any tool (left-drag draws boxes with Box select).
            WindowEvent::MouseInput {
                state,
                button: MouseButton::Right,
                ..
            } => self.dragging = state == ElementState::Pressed,
            WindowEvent::CursorMoved { position, .. } => {
                if let (Some((start, base)), Some(reg)) = (self.nudge_drag, self.reg.as_mut()) {
                    let now = renderer.screen_to_world([position.x as f32, position.y as f32]);
                    reg.drag_nudge = Some([base[0] + now[0] - start[0], base[1] + now[1] - start[1]]);
                } else if self.dragging {
                    let (dx, dy) = (position.x - self.cursor.x, position.y - self.cursor.y);
                    renderer.pan(dx as f32, dy as f32);
                }
                self.cursor = position;
            }
            WindowEvent::MouseWheel { delta, .. } => {
                // ponytail: fixed zoom speeds, make them a preference if trackpads feel off
                let factor = match delta {
                    MouseScrollDelta::LineDelta(_, y) => 1.15_f32.powf(-y),
                    MouseScrollDelta::PixelDelta(p) => (-p.y as f32 * 0.003).exp(),
                };
                renderer.zoom(factor, [self.cursor.x as f32, self.cursor.y as f32]);
            }
            WindowEvent::ScaleFactorChanged { .. } => {
                let scale_factor = window.scale_factor() as f32;
                gui_state.egui_ctx().set_pixels_per_point(scale_factor);
            }
            WindowEvent::Resized(PhysicalSize { width, height }) => {
                if width == 0 || height == 0 {
                    return;
                }

                log::info!("Resizing renderer surface to: ({width}, {height})");
                renderer.resize(width, height);
                self.last_size = (width, height);

                let scale_factor = window.scale_factor() as f32;
                gui_state.egui_ctx().set_pixels_per_point(scale_factor);
            }
            WindowEvent::CloseRequested => close_requested = true,
            WindowEvent::RedrawRequested => {
                // Get time delta sice last draw
                let now = Instant::now();
                let delta_time = now - *last_render_time;
                *last_render_time = now;
                let frame_ms = delta_time.as_secs_f64() * 1000.0;
                self.dev.timings.frame_ms = 0.9 * self.dev.timings.frame_ms + 0.1 * frame_ms;

                //take gui input
                let gui_input = gui_state.take_egui_input(window);
                let mut actions = ui::UiActions::default();
                let ppp = gui_state.egui_ctx().pixels_per_point();
                let labels = self
                    .store
                    .as_ref()
                    .filter(|_| self.layers.node_ids || self.layers.hyperedge_ids)
                    .and_then(|store| id_labels(store, &self.layers, renderer, ppp));
                let selected_added = match self.selection {
                    Some(ui::Selection::Added(id)) => Some(id),
                    _ => None,
                };
                let to_pt = |p: [f32; 2]| {
                    let [x, y] = renderer.world_to_screen(p);
                    egui::pos2(x / ppp, y / ppp)
                };
                let registering = self.workflow == registration::Workflow::Registration;
                let added_segments =
                    self.store.as_ref().map_or_else(Vec::new, |s| added_segments(s, self.annot.as_ref()));
                let overlay = match (registering, self.store.as_ref(), self.annot.as_ref(), self.reg.as_mut()) {
                    (true, Some(store), _, Some(reg)) => registration::overlay(reg, store, &added_segments, to_pt),
                    (false, Some(store), Some(st), _) => annotate::overlay(store, st, renderer.time, selected_added, to_pt),
                    _ => vec![],
                };
                let mut overlay = overlay;
                if let Some(store) = self.store.as_ref() {
                    overlay.extend(detached_overlay(store, renderer, to_pt));
                }
                let mut map_texts = vec![];
                if let (false, Some(store), Some(st)) = (registering, self.store.as_ref(), self.annot.as_ref()) {
                    let cursor = [self.cursor.x as f32, self.cursor.y as f32];
                    let (trail_shapes, texts) = trail_overlay(store, st, &self.trail_routes, renderer, ppp);
                    overlay.extend(trail_shapes);
                    map_texts = texts;
                    overlay.extend(tool_overlay(store, st, renderer, ppp, cursor, self.box_start));
                }
                let unreachable_trails: Vec<String> = self
                    .annot
                    .as_ref()
                    .map(|st| {
                        st.own
                            .trails
                            .iter()
                            .filter(|(name, w)| {
                                w.len() >= 2
                                    && self.trail_routes.get(&(st.author.clone(), (*name).clone())).is_some_and(|r| r.2.is_none())
                            })
                            .map(|(name, _)| name.clone())
                            .collect()
                    })
                    .unwrap_or_default();
                if let Some(timeline) = self.timeline.as_mut() {
                    timeline.advance(delta_time.as_secs_f32());
                }

                //TODO: move this title to a constant or config file
                let title = "GrAMF - Graphs for Arbuscular Mycorrhizal Fungi";

                // Build the egui UI
                let egui_winit::egui::FullOutput {
                    //textures_delta is used to track changes to textures that egui uses (e.g., for images in the UI). It contains information about which textures were added, changed, or removed during this frame.
                    textures_delta,
                    shapes,
                    pixels_per_point,
                    platform_output,
                    ..
                } = gui_state.egui_ctx().run_ui(gui_input, |ui| {
                    // Build the UI using the `build_ui` function defined in the `ui` module
                    actions = ui::build_ui(
                        ui,
                        ui::FrameState {
                            title,
                            status: &self.status,
                            timeline: self.timeline.as_mut(),
                            layers: &mut self.layers,
                            store: self.store.as_ref(),
                            selection: self.selection,
                            labels: labels.as_deref(),
                            annot: self.annot.as_mut(),
                            overlay: &overlay,
                            confirm: self.confirm.is_some(),
                            export: &mut self.export,
                            dev: &mut self.dev,
                            update: self.update.as_deref(),
                            raw: self.raw.as_ref(),
                            boxed: &self.boxed,
                            reveal: self.reveal.is_some(),
                            settings_window: &mut self.settings_window,
                            unreachable_trails: &unreachable_trails,
                            map_texts: &map_texts,
                            workflow: &mut self.workflow,
                            added_segments: &added_segments,
                            reg: self.reg.as_mut(),
                        },
                    );
                });

                // Handle any platform output from egui (e.g., opening URLs, copying to clipboard, etc.)
                gui_state.handle_platform_output(window, platform_output);

                // Tessellate the egui shapes into paint jobs that can be rendered by the renderer
                let paint_jobs = gui_state.egui_ctx().tessellate(shapes, pixels_per_point);

                // Get descriptor for current screen
                let screen_descriptor = {
                    let (width, height) = self.last_size;
                    if width == 0 || height == 0 {
                        return;
                    }
                    egui_wgpu::ScreenDescriptor {
                        size_in_pixels: [width, height],
                        pixels_per_point,
                    }
                };

                // In registration the STG time follows the selected run.
                if let (registration::Workflow::Registration, Some(reg), Some(timeline)) =
                    (self.workflow, self.reg.as_ref(), self.timeline.as_mut())
                {
                    if let Some(f) = reg.selected_run_utc().and_then(|t| crate::video::runs::stg_frame(t, &timeline.axis.timepoints)) {
                        timeline.index = f;
                    }
                }
                if let Some(timeline) = &self.timeline {
                    renderer.time = timeline.index as f32;
                }
                if let Some(store) = self.store.as_ref() {
                    let key = (self.timeline.as_ref().map(|t| t.index), self.layers.colour_by, self.layers.scale);
                    if self.style_key != Some(key) {
                        renderer.set_edge_styles(&crate::colour::edge_styles(store, key.0, key.2));
                        self.style_key = Some(key);
                    }
                    renderer.line = crate::renderer::LineStyle {
                        base_px: self.layers.line_px,
                        by_width: self.layers.line_by_width,
                        edge_colour: self.layers.colour_by == crate::colour::ColourBy::Width,
                        reference_width_um: self.reference_width,
                    };
                }

                // Map area must be known before rendering (fit uses its aspect)
                if let Some(r) = actions.map_rect.map(|r| r * pixels_per_point) {
                    renderer.set_map_rect([r.min.x, r.min.y, r.width(), r.height()]);
                }

                // Render frame
                /*
                Screen descriptor has width, height and dpi for all renderers to use
                Paint jobs and textures delta are used to render the egui UI
                delta_time is used to update the scene based on time (e.g., for animations)

                This function will render the graph scene, then the EGUI ui                
                 */
                renderer.render_frame(screen_descriptor, paint_jobs, textures_delta, delta_time);
                ui_actions = Some(actions);
            }
            _ => (),
        }

        // Request a redraw of the window after processing the event to ensure that the UI is updated
        window.request_redraw();

        if escape {
            self.box_start = None;
            let cancelled_step = self.annot.as_mut().and_then(|a| a.first.take()).is_some();
            if !cancelled_step {
                self.select(None);
            }
        }
        if let Some(px) = click {
            self.pick(px);
        }
        if let Some((a, b)) = boxed {
            self.box_select(a, b);
        }
        if let Some(actions) = ui_actions {
            self.apply(actions);
        }
        if close_requested {
            self.request_quit();
        }
        #[cfg(target_os = "macos")]
        self.handle_native_menu();
        // Registration edits go through the annotation file (undo, dirty, save) and come
        // back into the workspace as its current state.
        if let (Some(reg), Some(st)) = (self.reg.as_mut(), self.annot.as_mut()) {
            for op in reg.pending.drain(..) {
                st.own.edit(op);
            }
            reg.sync(&st.own.registration);
        }
        if let Some(path) = self.pending_open.take() {
            self.open_store(&path);
        }
        self.refresh_dev();
        self.refresh_trail_routes();
        if let Some(tag) = self.update_check.as_ref().and_then(|rx| rx.try_recv().ok()) {
            self.update = Some(tag);
        }

        let title = match (&self.store_path, self.dirty()) {
            (Some(p), dirty) => format!("{}GrAMF — {}", if dirty { "• " } else { "" }, p.display()),
            (None, _) => "GrAMF".into(),
        };
        if title != self.window_title {
            if let Some(window) = self.window.as_ref() {
                window.set_title(&title);
            }
            self.window_title = title;
        }
        if self.quit {
            event_loop.exit();
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn clip_segment_to_box() {
        let (min, max) = ([0.0, 0.0], [10.0, 10.0]);
        assert_eq!(super::clip_segment([-10.0, 5.0], [20.0, 5.0], min, max), Some(([0.0, 5.0], [10.0, 5.0])));
        assert_eq!(super::clip_segment([2.0, 2.0], [3.0, 3.0], min, max), Some(([2.0, 2.0], [3.0, 3.0])));
        assert_eq!(super::clip_segment([-5.0, 20.0], [20.0, 20.0], min, max), None);
        assert_eq!(super::clip_segment([-5.0, -5.0], [-1.0, 15.0], min, max), None);
    }
}
