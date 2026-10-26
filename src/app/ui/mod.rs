use super::annotate::{self, AnnotationState, Choice};
use super::dev_panel::{self, DevState};
use super::export_dialog::{self, ExportDialog};
use super::registration::{self, Registration, Workflow};
use super::settings_window::{self, SettingsWindow};
use crate::annotations::{Op, Target};
use crate::colour::{Cmap, ColourBy, Scale};
use crate::store::Store;
use crate::time::{TimeAxis, TimeUnit};

mod inspector;
mod legend;
mod timeline;

use inspector::inspector;
use legend::legend;
pub use timeline::Timeline;
use timeline::timeline_bar;

/// Room right of the slider for the time label, faulty-frame marker and unit picker (points).
const TIME_LABEL_WIDTH: f32 = 420.0;

/// File / Edit / View live in the screen's menu bar on macOS, in the window elsewhere.
const NATIVE_MENU: bool = cfg!(target_os = "macos");

#[derive(Clone, Copy, PartialEq)]
pub enum Selection {
    /// `edge_id`
    Edge(usize),
    /// Row in `/nodes`
    Node(usize),
    /// Hyperedge id
    Hyperedge(i64),
    /// Id of an edge added in an annotation file
    Added(i64),
    /// The edges inside the last box drawn with the Box select tool
    Box,
}

pub struct Layers {
    pub colour_by: ColourBy,
    /// Colormap for the continuous modes (t_first, width).
    pub scale: Scale,
    /// Line thickness in pixels, and whether it follows each edge's width at the current time.
    pub line_px: f32,
    pub line_by_width: bool,
    /// Side panels shown (left: Layers, right: Inspector).
    pub left_open: bool,
    pub right_open: bool,
    pub node_ids: bool,
    pub hyperedge_ids: bool,
}

impl Default for Layers {
    fn default() -> Self {
        Self {
            colour_by: ColourBy::Uniform,
            scale: Scale::default(),
            line_px: 1.5,
            line_by_width: false,
            left_open: true,
            right_open: true,
            node_ids: false,
            hyperedge_ids: false,
        }
    }
}

/// Everything one UI frame shows or edits.
pub struct FrameState<'a> {
    pub title: &'a str,
    pub status: &'a str,
    pub timeline: Option<&'a mut Timeline>,
    pub layers: &'a mut Layers,
    pub store: Option<&'a Store>,
    pub selection: Option<Selection>,
    /// Id labels, positioned in points; `None` when there are too many to show.
    pub labels: Option<&'a [(egui::Pos2, String)]>,
    pub annot: Option<&'a mut AnnotationState>,
    /// Annotation overlay, in points.
    pub overlay: &'a [egui::Shape],
    /// Ask what to do with unsaved annotations.
    pub confirm: bool,
    pub export: &'a mut ExportDialog,
    pub dev: &'a mut DevState,
    /// Tag of a newer release, if the startup check found one.
    pub update: Option<&'a str>,
    /// Raw rows loaded for this edge, if any.
    pub raw: Option<&'a (usize, Vec<crate::devtools::RawTable>)>,
    /// Edges of the box selection.
    pub boxed: &'a [usize],
    /// The status line names a file that can be shown in the file manager.
    pub reveal: bool,
    pub settings_window: &'a mut SettingsWindow,
    /// The user's trails with no connecting path.
    pub unreachable_trails: &'a [String],
    /// Text drawn on the map (e.g. trail names), in points.
    pub map_texts: &'a [(egui::Pos2, String, egui::Color32)],
    pub workflow: &'a mut Workflow,
    /// The user's added edges as segments, for matching video edges onto them.
    pub added_segments: &'a [crate::video::matching::AddedSegment],
    /// The plate's video runs, once their folder is chosen.
    pub reg: Option<&'a mut Registration>,
}

/// What the user asked for this frame; acted on outside the egui closure.
#[derive(Default)]
pub struct UiActions {
    pub open: bool,
    pub colour_changed: bool,
    pub select: Option<Selection>,
    pub ops: Vec<Op>,
    pub save: bool,
    pub undo: bool,
    pub redo: bool,
    pub open_settings: bool,
    pub settings_saved: Option<crate::settings::Settings>,
    pub choice: Option<Choice>,
    pub export: bool,
    pub evidence_toggled: bool,
    /// Pick the folder with the plate's video runs.
    pub choose_videos: bool,
    pub detect_camera: bool,
    pub save_setup_default: bool,
    pub publish: bool,
    pub fit: bool,
    /// Keyboard zoom by this factor (< 1: in).
    pub zoom: Option<f32>,
    pub zoom_selection: bool,
    /// The inspector's Raw rows section is open: keep the rows for the selected edge loaded.
    pub raw_wanted: bool,
    pub reveal: bool,
    /// Area left over between the panels, in points: where the map is drawn.
    pub map_rect: Option<egui::Rect>,
}

pub fn build_ui(ui: &mut egui::Ui, frame: FrameState) -> UiActions {
    let FrameState { title, status, timeline, layers, store, selection, labels, mut annot, overlay, confirm, export, dev, update, raw, boxed, reveal, settings_window, unreachable_trails, map_texts, workflow, added_segments, mut reg } =
        frame;
    let registering = *workflow == Workflow::Registration;
    let mut actions = UiActions::default();

    // Shortcuts; single-letter tool keys only while no text field has focus. On macOS the
    // menu bar owns ⌘S / ⌘Z, so handling them here too would act twice.
    if !NATIVE_MENU {
        ui.input(|i| {
            actions.save = i.modifiers.command && i.key_pressed(egui::Key::S);
            actions.undo = i.modifiers.command && !i.modifiers.shift && i.key_pressed(egui::Key::Z);
            actions.redo = i.modifiers.command && i.modifiers.shift && i.key_pressed(egui::Key::Z);
            if i.modifiers.command && (i.key_pressed(egui::Key::Equals) || i.key_pressed(egui::Key::Plus)) {
                actions.zoom = Some(1.0 / super::ZOOM_STEP);
            }
            if i.modifiers.command && i.key_pressed(egui::Key::Minus) {
                actions.zoom = Some(super::ZOOM_STEP);
            }
            actions.fit |= i.modifiers.command && i.key_pressed(egui::Key::Num0);
            actions.zoom_selection = i.modifiers.command && i.key_pressed(egui::Key::Num2);
        });
    }
    if let (true, Some(r)) = (registering && !ui.ctx().egui_wants_keyboard_input(), reg.as_deref_mut()) {
        for (tool, key, _) in registration::RegTool::ALL {
            if ui.input(|i| !i.modifiers.command && i.key_pressed(key)) {
                r.tool = tool;
            }
        }
    }
    if let Some(st) = annot.as_deref_mut().filter(|_| !registering && !ui.ctx().egui_wants_keyboard_input()) {
        for tool in annotate::Tool::ALL {
            if ui.input(|i| !i.modifiers.command && i.key_pressed(tool.key())) {
                st.tool = tool;
                st.first = None;
            }
        }
    }
    if confirm {
        if let Some(st) = annot.as_deref() {
            actions.choice = annotate::confirm(ui, &st.author);
        }
    }
    if settings_window.is_open() {
        let author_locked = annot.as_deref().is_some_and(|st| st.own.dirty);
        actions.settings_saved = settings_window::show(ui.ctx(), settings_window, author_locked);
    }
    if export.open {
        let authors: Vec<String> = annot
            .as_deref()
            .map(|st| std::iter::once(st.author.clone()).chain(st.others.iter().map(|(a, _)| a.author.clone())).collect())
            .unwrap_or_default();
        actions.export = export_dialog::show(ui.ctx(), export, &authors, selection.is_some());
    }
    egui::Panel::top("top").show_inside(ui, |ui| {
        ui.horizontal(|ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                // macOS has these in the screen's menu bar (native_menu.rs) instead.
                if !NATIVE_MENU {
                    ui.menu_button("File", |ui| {
                        if ui.button("Open store…").clicked() {
                            actions.open = true;
                            ui.close();
                        }
                        if ui.add(egui::Button::new("Save annotations").shortcut_text("Ctrl+S")).clicked() {
                            actions.save = true;
                            ui.close();
                        }
                        if ui.add_enabled(store.is_some(), egui::Button::new("Export tables…")).clicked() {
                            export.open = true;
                            ui.close();
                        }
                        ui.separator();
                        if ui.button("Settings…").clicked() {
                            actions.open_settings = true;
                            ui.close();
                        }
                    });
                    ui.menu_button("Edit", |ui| {
                        if ui.add(egui::Button::new("Undo").shortcut_text("Ctrl+Z")).clicked() {
                            actions.undo = true;
                            ui.close();
                        }
                        if ui.add(egui::Button::new("Redo").shortcut_text("Ctrl+Shift+Z")).clicked() {
                            actions.redo = true;
                            ui.close();
                        }
                    });
                    ui.menu_button("View", |ui| {
                        for (text, keys, zoom) in [("Zoom In", "Ctrl+=", 1.0 / super::ZOOM_STEP), ("Zoom Out", "Ctrl+-", super::ZOOM_STEP)] {
                            if ui.add(egui::Button::new(text).shortcut_text(keys)).clicked() {
                                actions.zoom = Some(zoom);
                                ui.close();
                            }
                        }
                        if ui.add(egui::Button::new("Reset Camera").shortcut_text("Ctrl+0 / Home")).clicked() {
                            actions.fit = true;
                            ui.close();
                        }
                        if ui.add(egui::Button::new("Zoom to Selection").shortcut_text("Ctrl+2")).clicked() {
                            actions.zoom_selection = true;
                            ui.close();
                        }
                        ui.separator();
                        ui.checkbox(&mut dev.open, "Developer view");
                    });
                    ui.separator();
                }

                ui.label(egui::RichText::new(title).color(egui::Color32::LIGHT_GREEN));

                ui.separator();
                let file_manager = if cfg!(windows) { "Show in Explorer" } else { "Show in Finder" };
                if reveal && ui.button(file_manager).clicked() {
                    actions.reveal = true;
                }
                ui.label(status);
            });

            ui.with_layout(egui::Layout::right_to_left(egui::Align::RIGHT), |ui| {
                ui.add_space(10.0);
                ui.label(egui::RichText::new(concat!("v", env!("CARGO_PKG_VERSION"))).color(egui::Color32::ORANGE));
                if let Some(tag) = update {
                    ui.hyperlink_to(format!("{tag} available"), crate::update::RELEASES_URL);
                }
                ui.separator();
            });
        });
        ui.separator();
        ui.horizontal(|ui| {
            ui.selectable_value(workflow, Workflow::Visualize, "Visualize and clean up");
            ui.selectable_value(workflow, Workflow::Registration, "Video registration");
            ui.separator();
            match (registering, annot.as_deref_mut()) {
                (false, Some(st)) => annotate::toolbar(ui, st),
                (true, _) => match reg.as_deref_mut() {
                    Some(r) => actions.publish = registration::toolbar(ui, r),
                    None => {
                        ui.label(egui::RichText::new("choose the plate's video runs folder (left panel)").weak());
                    }
                },
                _ => (),
            }
        });
    });

    // Registration: the run slider replaces the timeline; the STG time follows the run.
    let timeline = match (registering, reg.as_deref_mut()) {
        (true, Some(r)) => {
            egui::Panel::bottom("runs").show_inside(ui, |ui| registration::run_bar(ui, r));
            timeline.map(|t| &*t)
        }
        _ => timeline.map(|t| {
            egui::Panel::bottom("timeline").show_inside(ui, |ui| timeline_bar(ui, t));
            &*t
        }),
    };

    if dev.open {
        egui::Panel::bottom("developer")
            .resizable(true)
            .default_size(240.0)
            .show_inside(ui, |ui| actions.evidence_toggled = dev_panel::show(ui, dev));
    }

    if !layers.left_open {
        egui::Panel::left("layers_collapsed").resizable(false).exact_size(22.0).show_inside(ui, |ui| {
            if ui.small_button("»").on_hover_text("show Layers").clicked() {
                layers.left_open = true;
            }
        });
    } else {
    egui::Panel::left("layers").show_inside(ui, |ui| {
        ui.horizontal(|ui| {
            if !registering {
                ui.heading("Layers");
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.small_button("«").on_hover_text("hide this panel").clicked() {
                    layers.left_open = false;
                }
            });
        });
        if registering {
            if let Some(store) = store {
                match registration::left_panel(ui, reg.as_deref_mut(), store, added_segments) {
                    registration::Action::ChooseFolder => actions.choose_videos = true,
                    registration::Action::DetectCamera => actions.detect_camera = true,
                    registration::Action::SaveDefault => actions.save_setup_default = true,
                    registration::Action::ZoomRun => actions.zoom_selection = true,
                    registration::Action::None => (),
                }
            }
            return;
        }
        let before = layers.colour_by;
        egui::ComboBox::from_label("colour")
            .selected_text(layers.colour_by.name())
            .show_ui(ui, |ui| {
                for by in ColourBy::ALL {
                    ui.selectable_value(&mut layers.colour_by, by, by.name());
                }
            });
        let scale_before = layers.scale;
        if let Some(store) = store {
            legend(ui, store, layers, timeline);
        }
        actions.colour_changed = layers.colour_by != before || layers.scale != scale_before;
        ui.horizontal(|ui| {
            ui.label("lines");
            ui.selectable_value(&mut layers.line_by_width, false, "thin");
            ui.selectable_value(&mut layers.line_by_width, true, "by width")
                .on_hover_text("thickness follows each edge's width at the current timepoint");
        });
        ui.add(egui::Slider::new(&mut layers.line_px, 0.5..=6.0).text("px").step_by(0.25));
        ui.checkbox(&mut layers.node_ids, "Node ids");
        ui.checkbox(&mut layers.hyperedge_ids, "Hyperedge ids");
        if (layers.node_ids || layers.hyperedge_ids) && labels.is_none() {
            ui.label(egui::RichText::new("zoom in to see ids").weak());
        }
        if let (Some(st), Some(store)) = (annot.as_deref_mut(), store) {
            ui.separator();
            match annotate::panel(ui, st, store, unreachable_trails) {
                annotate::PanelAction::Ops(ops) => actions.ops.extend(ops),
                annotate::PanelAction::OpenSettings => actions.open_settings = true,
                annotate::PanelAction::None => (),
            }
        }
    });

    }

    if !layers.right_open {
        egui::Panel::right("inspector_collapsed").resizable(false).exact_size(22.0).show_inside(ui, |ui| {
            if ui.small_button("«").on_hover_text("show Inspector").clicked() {
                layers.right_open = true;
            }
        });
    } else {
    egui::Panel::right("right").show_inside(ui, |ui| {
        ui.horizontal(|ui| {
            if ui.small_button("»").on_hover_text("hide Inspector").clicked() {
                layers.right_open = false;
            }
            ui.heading(if registering { "Run" } else { "Inspector" });
        });
        ui.separator();
        if registering {
            let unit = timeline.map_or(TimeUnit::Utc, |t| t.unit);
            if let Some(store) = store {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    actions.zoom_selection |= registration::right_panel(ui, reg.as_deref_mut(), timeline.map(|t| &t.axis), unit, store, added_segments);
                });
            }
            return;
        }
        match (store, selection) {
            (Some(store), Some(sel)) => {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    let (select, ops) =
                        inspector(ui, store, sel, timeline, annot.as_deref_mut(), raw, &mut actions.raw_wanted, boxed);
                    actions.select = select;
                    actions.ops.extend(ops);
                });
            }
            _ => {
                ui.label(egui::RichText::new("Click an edge or node").weak());
            }
        }
    });
    }

    let map = ui.available_rect_before_wrap();
    let painter = ui.painter_at(map);
    painter.extend(overlay.iter().cloned());
    for (pos, text, colour) in map_texts {
        painter.text(*pos, egui::Align2::LEFT_BOTTOM, text, egui::FontId::proportional(12.0), *colour);
    }
    if let Some(labels) = labels {
        for (pos, text) in labels {
            painter.text(
                *pos,
                egui::Align2::CENTER_CENTER,
                text,
                egui::FontId::monospace(11.0),
                egui::Color32::WHITE,
            );
        }
    }
    actions.map_rect = Some(map);
    actions
}
