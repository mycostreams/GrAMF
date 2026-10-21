//! What App does with files and settings: open a store, its annotations and video runs, save,
//! publish, export, and quit.

use super::*;

/// Shows `path` selected in Finder / Explorer (its folder elsewhere).
pub(super) fn reveal_in_file_manager(path: &Path) {
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

impl App {
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
    pub(super) fn camera_and_default(&mut self, detect: bool, save: bool) {
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
    pub(super) fn choose_videos(&mut self) {
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
    pub(super) fn apply_settings(&mut self, new: crate::settings::Settings) {
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

    pub(super) fn open_store(&mut self, path: &Path) {
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
        // Video runs beside the store (or one folder up, for <plate>/data/stg_store.h5), named
        // polyflow_analysis (registrator) or polyflow_data (paper_plates sessions).
        self.reg = path
            .ancestors()
            .skip(1)
            .take(2)
            .flat_map(|dir| [dir.join("polyflow_analysis"), dir.join("polyflow_data")])
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
    pub(super) fn refresh_dev(&mut self) {
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

    pub(super) fn save_annotations(&mut self) -> bool {
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
    pub(super) fn publish_to_store(&mut self) {
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

    pub(super) fn export_tables(&mut self) {
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
    pub(super) fn refresh_trail_routes(&mut self) {
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
    pub(super) fn request_quit(&mut self) {
        if self.dirty() {
            self.confirm = Some(Pending::Quit);
        } else {
            self.quit = true;
        }
    }

    /// Acts on the macOS menu bar.
    #[cfg(target_os = "macos")]
    pub(super) fn handle_native_menu(&mut self) {
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
                Command::ZoomIn => actions.zoom = Some(1.0 / ZOOM_STEP),
                Command::ZoomOut => actions.zoom = Some(ZOOM_STEP),
                Command::ZoomSelection => actions.zoom_selection = true,
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
    pub(super) fn proceed(&mut self, pending: Pending) {
        if let Some(st) = self.annot.as_mut() {
            st.own.dirty = false; // saved or discarded by now
        }
        match pending {
            Pending::Quit => self.quit = true,
            Pending::Open(path) => self.pending_open = Some(path),
        }
    }
}
