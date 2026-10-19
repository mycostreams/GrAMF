mod annotate;
mod export_dialog;
mod ui;

use crate::annotations::{self, AddedEdge, Annotations, Op};
use crate::colour::vertex_colours;
use annotate::{AnnotationState, Pending, Tool};
use crate::registry;
use crate::renderer::AppRenderer;
use crate::store::Store;
use crate::time::TimeAxis;
use egui_winit::State;
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
}

/// Click tolerance and the most id labels drawn at once.
const PICK_RADIUS_PX: f32 = 6.0;
const MAX_LABELS: usize = 1500;

impl App {
    pub fn new(open: Option<PathBuf>) -> Self {
        Self {
            pending_open: open,
            ..Default::default()
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
        let store = match crate::store::read_store(path) {
            Ok(store) => store,
            Err(e) => {
                self.status = format!("Could not open {}: {e}", path.display());
                return;
            }
        };
        let n_timepoints = store.timepoints.len();
        renderer.load_store(&store, &vertex_colours(&store, self.layers.colour_by, n_timepoints));
        self.selection = None;

        let candidates = registry::plate_candidates(path);
        let plate = registry::default_dir().and_then(|dir| registry::lookup(&dir, &candidates));
        let plate_status = match &plate {
            Some(p) => p.ui.clone(),
            None if candidates.is_empty() => "plate id not found in path".into(),
            None => format!("{} not in registry", candidates[0]),
        };
        self.status = format!("{plate_status} · {} edges", store.arcs.n_edges());
        self.plate = plate
            .as_ref()
            .map(|p| p.ui.clone())
            .or_else(|| candidates.first().cloned())
            .unwrap_or_else(|| "plate".into());

        self.timeline = TimeAxis::new(
            store.timepoints.clone(),
            store.quality.clone(),
            plate.as_ref().and_then(|p| p.inoculation),
            plate.as_ref().and_then(|p| p.crossed),
        )
        .map(ui::Timeline::new);
        renderer.time = self.timeline.as_ref().map_or(f32::MAX, |t| t.index as f32);

        let (annot, errors) = load_annotations(path, &store, &annotate::load_author());
        if !errors.is_empty() {
            self.status = format!("{} · could not read {}", self.status, errors.join(", "));
        }
        self.annot = Some(annot);
        self.store_path = Some(path.to_path_buf());
        self.store = Some(store);
    }

    fn save_annotations(&mut self) -> bool {
        let Some(st) = self.annot.as_mut() else {
            return false;
        };
        match annotations::save(&st.own, &st.own_path) {
            Ok(()) => {
                st.own.dirty = false;
                self.status = format!("Saved {}", st.own_path.display());
                true
            }
            Err(e) => {
                self.status = format!("Could not save {}: {e}", st.own_path.display());
                false
            }
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
        self.status = match crate::export::export(&folder, &req) {
            Ok(dir) => format!("Exported to {}", dir.display()),
            Err(e) => format!("Export failed: {e}"),
        };
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
            Some(ui::Selection::Added(_)) | None => vec![], // the overlay highlights added edges
        };
        renderer.set_selection(store, &edges);
    }

    /// Acts on a click at window pixel `px` with the current tool.
    fn pick(&mut self, px: [f32; 2]) {
        let (Some(renderer), Some(store), Some(st)) = (self.renderer.as_ref(), self.store.as_ref(), self.annot.as_mut())
        else {
            return;
        };
        let at = renderer.screen_to_world(px);
        let radius = PICK_RADIUS_PX * renderer.um_per_px();
        let t = renderer.time;
        let node = || store.pick_node(at, radius, t);
        let edge = || store.pick_edge(at, radius, t);

        let mut op = None;
        let selection = match st.tool {
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

    fn apply(&mut self, actions: ui::UiActions) {
        if actions.colour_changed {
            if let (Some(renderer), Some(store)) = (self.renderer.as_mut(), self.store.as_ref()) {
                let colours = vertex_colours(store, self.layers.colour_by, store.timepoints.len());
                renderer.set_colours(store, &colours);
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
        if let Some(name) = actions.switch_author {
            if let (Some(path), Some(store)) = (self.store_path.as_ref(), self.store.as_ref()) {
                annotate::save_author(&name);
                let (annot, _) = load_annotations(path, store, &name);
                self.annot = Some(annot);
            }
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
        if actions.open {
            self.pending_open = rfd::FileDialog::new()
                .add_filter("STG store", &["h5"])
                .pick_file();
        }
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
        let mut escape = false;

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
                self.dragging = state == ElementState::Pressed;
                if self.dragging {
                    self.press = Some(self.cursor);
                } else if let Some(p) = self.press.take() {
                    if (p.x - self.cursor.x).hypot(p.y - self.cursor.y) < 4.0 {
                        click = Some([self.cursor.x as f32, self.cursor.y as f32]);
                    }
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                if self.dragging {
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
            WindowEvent::CloseRequested => {
                if self.annot.as_ref().is_some_and(|a| a.own.dirty) {
                    self.confirm = Some(Pending::Quit);
                } else {
                    event_loop.exit();
                }
            }
            WindowEvent::RedrawRequested => {
                // Get time delta sice last draw
                let now = Instant::now();
                let delta_time = now - *last_render_time;
                *last_render_time = now;

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
                let overlay = match (self.store.as_ref(), self.annot.as_ref()) {
                    (Some(store), Some(st)) => annotate::overlay(store, st, renderer.time, selected_added, |p| {
                        let [x, y] = renderer.world_to_screen(p);
                        egui::pos2(x / ppp, y / ppp)
                    }),
                    _ => vec![],
                };
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

                if let Some(timeline) = &self.timeline {
                    renderer.time = timeline.index as f32;
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
            let cancelled_step = self.annot.as_mut().and_then(|a| a.first.take()).is_some();
            if !cancelled_step {
                self.select(None);
            }
        }
        if let Some(px) = click {
            self.pick(px);
        }
        if let Some(actions) = ui_actions {
            self.apply(actions);
        }
        if let Some(path) = self.pending_open.take() {
            self.open_store(&path);
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
