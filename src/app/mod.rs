mod annotate;
mod dev_panel;
mod export_dialog;
#[cfg(target_os = "macos")]
mod native_menu;
mod registration;
mod settings_window;
mod ui;

mod actions;
mod overlays;
mod picking;
mod window;

use actions::reveal_in_file_manager;
use overlays::{detached_overlay, id_labels, tool_overlay, trail_overlay};

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
    /// The open plate's video runs (Video registration workflow), once the user has chosen their folder.
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
    /// Keyboard modifiers held, for scroll zoom.
    modifiers: winit::keyboard::ModifiersState,
    /// Space is held: left-drag pans (a tap without a drag plays / pauses).
    space_held: bool,
    space_panned: bool,
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
/// Keyboard zoom step (⌘= / ⌘−).
const ZOOM_STEP: f32 = 1.5;
/// Scroll-wheel notch in pixels, when panning with a mouse wheel.
const LINE_PX: f32 = 40.0;

impl App {
    pub fn new(open: Option<PathBuf>) -> Self {
        Self {
            pending_open: open,
            settings: crate::settings::Settings::load(),
            ..Default::default()
        }
    }

    fn dirty(&self) -> bool {
        self.annot.as_ref().is_some_and(|a| a.own.dirty)
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
        if let (Some(factor), Some(renderer)) = (actions.zoom, self.renderer.as_mut()) {
            renderer.zoom_keyboard(factor, [self.cursor.x as f32, self.cursor.y as f32]);
        }
        if actions.zoom_selection {
            if let (Some((min, max)), Some(renderer)) = (self.selection_bounds(), self.renderer.as_mut()) {
                renderer.fit_to(min, max);
            }
        }
        if actions.open {
            self.pending_open = rfd::FileDialog::new()
                .add_filter("STG store", &["h5"])
                .pick_file();
        }
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
