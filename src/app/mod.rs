mod ui;

use crate::colour::vertex_colours;
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

    fn open_store(&mut self, path: &Path) {
        let (Some(renderer), Some(window)) = (self.renderer.as_mut(), self.window.as_ref()) else {
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
        window.set_title(&format!("GrAMF — {}", path.display()));

        let candidates = registry::plate_candidates(path);
        let plate = registry::default_dir().and_then(|dir| registry::lookup(&dir, &candidates));
        let plate_status = match &plate {
            Some(p) => p.ui.clone(),
            None if candidates.is_empty() => "plate id not found in path".into(),
            None => format!("{} not in registry", candidates[0]),
        };
        self.status = format!("{plate_status} · {} edges", store.arcs.n_edges());

        self.timeline = TimeAxis::new(
            store.timepoints.clone(),
            store.quality.clone(),
            plate.as_ref().and_then(|p| p.inoculation),
            plate.as_ref().and_then(|p| p.crossed),
        )
        .map(ui::Timeline::new);
        renderer.time = self.timeline.as_ref().map_or(f32::MAX, |t| t.index as f32);
        self.store = Some(store);
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
            None => vec![],
        };
        renderer.set_selection(store, &edges);
    }

    /// Selects the node, else the edge, under window pixel `px`; empty space clears.
    fn pick(&mut self, px: [f32; 2]) {
        let (Some(renderer), Some(store)) = (self.renderer.as_ref(), self.store.as_ref()) else {
            return;
        };
        let at = renderer.screen_to_world(px);
        let radius = PICK_RADIUS_PX * renderer.um_per_px();
        let picked = store
            .pick_node(at, radius, renderer.time)
            .map(ui::Selection::Node)
            .or_else(|| store.pick_edge(at, radius, renderer.time).map(ui::Selection::Edge));
        self.select(picked);
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
        if actions.open {
            self.pending_open = rfd::FileDialog::new()
                .add_filter("STG store", &["h5"])
                .pick_file();
        }
    }
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
                (winit::keyboard::KeyCode::Escape, _) => event_loop.exit(),
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
                log::info!("Close requested. Exiting...");
                event_loop.exit();
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

        if let Some(px) = click {
            self.pick(px);
        }
        if let Some(actions) = ui_actions {
            self.apply(actions);
        }
        if let Some(path) = self.pending_open.take() {
            self.open_store(&path);
        }
    }
}
