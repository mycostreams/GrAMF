use super::annotate::{self, AnnotationState, Choice};
use super::dev_panel::{self, DevState};
use super::export_dialog::{self, ExportDialog};
use crate::annotations::{Op, Target};
use crate::colour::{Cmap, ColourBy, Scale};
use crate::store::Store;
use crate::time::{TimeAxis, TimeUnit};

/// Room right of the slider for the time label, faulty-frame marker and unit picker (points).
const TIME_LABEL_WIDTH: f32 = 420.0;

/// File / Edit / View live in the screen's menu bar on macOS, in the window elsewhere.
const NATIVE_MENU: bool = cfg!(target_os = "macos");

/// Seconds per frame when playing.
const PLAY_STEP_S: f32 = 0.25;

/// Time slider state for the loaded store.
pub struct Timeline {
    pub axis: TimeAxis,
    pub index: usize,
    pub unit: TimeUnit,
    pub playing: bool,
    play_accum: f32,
}

impl Timeline {
    /// Starts on the last frame, so a freshly opened plate shows its whole network.
    pub fn new(axis: TimeAxis) -> Self {
        let unit = if axis.available(TimeUnit::HoursSinceInoculation) {
            TimeUnit::HoursSinceInoculation
        } else {
            TimeUnit::Utc
        };
        Self {
            index: axis.len() - 1,
            axis,
            unit,
            playing: false,
            play_accum: 0.0,
        }
    }

    pub fn step(&mut self, delta: isize) {
        self.index = self.index.saturating_add_signed(delta).min(self.axis.len() - 1);
    }

    pub fn toggle_play(&mut self) {
        if !self.playing && self.index == self.axis.len() - 1 {
            self.index = 0;
        }
        self.playing = !self.playing;
    }

    /// Advances playback by `dt` seconds; stops on the last frame.
    pub fn advance(&mut self, dt: f32) {
        if !self.playing {
            return;
        }
        self.play_accum += dt;
        while self.play_accum >= PLAY_STEP_S {
            self.play_accum -= PLAY_STEP_S;
            self.step(1);
        }
        if self.index == self.axis.len() - 1 {
            self.playing = false;
        }
    }
}

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
    pub switch_author: Option<String>,
    pub choice: Option<Choice>,
    pub export: bool,
    pub evidence_toggled: bool,
    pub fit: bool,
    /// The inspector's Raw rows section is open: keep the rows for the selected edge loaded.
    pub raw_wanted: bool,
    pub reveal: bool,
    /// Area left over between the panels, in points: where the map is drawn.
    pub map_rect: Option<egui::Rect>,
}

pub fn build_ui(ui: &mut egui::Ui, frame: FrameState) -> UiActions {
    let FrameState { title, status, timeline, layers, store, selection, labels, mut annot, overlay, confirm, export, dev, update, raw, boxed, reveal } =
        frame;
    let mut actions = UiActions::default();

    // Shortcuts; single-letter tool keys only while no text field has focus. On macOS the
    // menu bar owns ⌘S / ⌘Z, so handling them here too would act twice.
    if !NATIVE_MENU {
        ui.input(|i| {
            actions.save = i.modifiers.command && i.key_pressed(egui::Key::S);
            actions.undo = i.modifiers.command && !i.modifiers.shift && i.key_pressed(egui::Key::Z);
            actions.redo = i.modifiers.command && i.modifiers.shift && i.key_pressed(egui::Key::Z);
        });
    }
    if let Some(st) = annot.as_deref_mut().filter(|_| !ui.ctx().egui_wants_keyboard_input()) {
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
                        if ui.add(egui::Button::new("Reset Camera").shortcut_text("Home")).clicked() {
                            actions.fit = true;
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
        if let Some(st) = annot.as_deref_mut() {
            ui.separator();
            annotate::toolbar(ui, st);
        }
    });

    let timeline = timeline.map(|t| {
        egui::Panel::bottom("timeline").show_inside(ui, |ui| timeline_bar(ui, t));
        &*t
    });

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
            ui.heading("Layers");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.small_button("«").on_hover_text("hide Layers").clicked() {
                    layers.left_open = false;
                }
            });
        });
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
            match annotate::panel(ui, st, store) {
                annotate::PanelAction::Ops(ops) => actions.ops.extend(ops),
                annotate::PanelAction::SwitchAuthor(name) => actions.switch_author = Some(name),
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
            ui.heading("Inspector");
        });
        ui.separator();
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

/// Property rows for the selection, then the annotation fields. Returns a new selection if
/// the user asked for one, and any annotation edits.
fn inspector(
    ui: &mut egui::Ui,
    store: &Store,
    sel: Selection,
    timeline: Option<&Timeline>,
    annot: Option<&mut AnnotationState>,
    raw: Option<&(usize, Vec<crate::devtools::RawTable>)>,
    raw_wanted: &mut bool,
    boxed: &[usize],
) -> (Option<Selection>, Vec<Op>) {
    let mut select = None;
    let mut ops = vec![];
    let row = |ui: &mut egui::Ui, key: &str, value: String| {
        ui.label(egui::RichText::new(key).weak());
        ui.label(value);
        ui.end_row();
    };
    // Two rows: the frame index, and the time in the slider's current unit.
    let appears = |ui: &mut egui::Ui, t: f32| {
        if !t.is_finite() {
            return row(ui, "appears", "—".into());
        }
        row(ui, "appears", format!("frame {}", t as usize));
        if let Some(tl) = timeline.filter(|tl| (t as usize) < tl.axis.len()) {
            if tl.unit != TimeUnit::Frame {
                row(ui, "", tl.axis.label(t as usize, tl.unit));
            }
        }
    };
    match sel {
        Selection::Edge(e) => {
            ui.strong(format!("Edge {e}"));
            egui::Grid::new("edge").num_columns(2).show(ui, |ui| {
                if let Some([u, v]) = store.edge_nodes.get(e) {
                    row(ui, "nodes", format!("{u} – {v}"));
                }
                row(ui, "length", format!("{:.1} µm", store.edge_length_um[e]));
                appears(ui, store.edge_t[e]);
                if let (false, Some(tl)) = (store.width_t.is_empty(), timeline) {
                    row(ui, "width now", match store.width_at(e, Some(tl.index)) {
                        Some(w) => format!("{w:.2} µm (frame {})", tl.index),
                        None => "not measured yet".into(),
                    });
                }
                row(ui, "width (pooled)", match store.width[e] {
                    Some((w, se)) => format!("{w:.2} ± {se:.2} µm"),
                    None => "not measured".into(),
                });
                let h = store.hyperedge[e];
                row(ui, "hyperedge", if h >= 0 { h.to_string() } else { "none".into() });
            });
            let h = store.hyperedge[e];
            if h >= 0 && ui.button(format!("Select hyperedge {h}")).clicked() {
                select = Some(Selection::Hyperedge(h));
            }
            if let Some(st) = annot {
                ops = annotate::inspector_section(ui, st, store, Target::Edge(e));
            }
            ui.separator();
            let section = egui::CollapsingHeader::new("Raw rows").id_salt("raw_rows").show(ui, |ui| match raw {
                Some((re, tables)) if *re == e => raw_tables(ui, tables),
                _ => {
                    ui.label(egui::RichText::new("loading…").weak());
                }
            });
            *raw_wanted = section.body_returned.is_some();
        }
        Selection::Node(n) => {
            let edges = store.incident_edges(n);
            ui.strong(format!("Node {}", store.node_ids[n]));
            egui::Grid::new("node").num_columns(2).show(ui, |ui| {
                let [x, y] = store.node_pos[n];
                row(ui, "position", format!("{x:.1}, {y:.1} µm"));
                row(ui, "degree", edges.len().to_string());
                appears(ui, store.node_t[n]);
            });
            ui.label(egui::RichText::new("edges").weak());
            ui.horizontal_wrapped(|ui| {
                for e in edges {
                    if ui.link(e.to_string()).clicked() {
                        select = Some(Selection::Edge(e));
                    }
                }
            });
            if let Some(st) = annot {
                ops = annotate::inspector_section(ui, st, store, Target::Node(store.node_ids[n]));
            }
        }
        Selection::Box => {
            let length: f32 = boxed.iter().map(|&e| store.edge_length_um[e]).sum();
            ui.strong("Box selection");
            egui::Grid::new("box").num_columns(2).show(ui, |ui| {
                row(ui, "edges", boxed.len().to_string());
                row(ui, "length", format!("{length:.1} µm"));
            });
            ui.label(egui::RichText::new("File > Export tables… can export just these edges").weak().small());
        }
        Selection::Added(id) => {
            ui.strong(format!("Added edge {id}"));
            let Some(st) = annot else {
                return (select, ops);
            };
            let mine = st.own.added.iter().find(|a| a.id == id).copied();
            let found = mine.map(|a| (st.author.clone(), a)).or_else(|| {
                st.others.iter().find_map(|(o, _)| o.added.iter().find(|a| a.id == id).map(|a| (o.author.clone(), *a)))
            });
            if let Some((author, a)) = found {
                egui::Grid::new("added").num_columns(2).show(ui, |ui| {
                    row(ui, "nodes", format!("{} – {}", a.u, a.v));
                    row(ui, "author", author);
                });
            }
            if let Some(a) = mine {
                if ui.button("Remove").clicked() {
                    ops.push(Op::RemoveAddedEdge(a));
                }
            }
        }
        Selection::Hyperedge(h) => {
            let edges = store.hyperedge_edges(h);
            let length: f32 = edges.iter().map(|&e| store.edge_length_um[e]).sum();
            let first = edges.iter().map(|&e| store.edge_t[e]).fold(f32::INFINITY, f32::min);
            ui.strong(format!("Hyperedge {h}"));
            egui::Grid::new("hyperedge").num_columns(2).show(ui, |ui| {
                row(ui, "edges", edges.len().to_string());
                row(ui, "length", format!("{length:.1} µm"));
                appears(ui, first);
            });
        }
    }
    (select, ops)
}

fn timeline_bar(ui: &mut egui::Ui, timeline: &mut Timeline) {
    ui.horizontal(|ui| {
        if ui.button("◀").on_hover_text("Previous frame (Left arrow)").clicked() {
            timeline.step(-1);
        }
        let play = if timeline.playing { "⏸" } else { "⏵" };
        if ui.button(play).on_hover_text("Play (Space)").clicked() {
            timeline.toggle_play();
        }
        if ui.button("▶").on_hover_text("Next frame (Right arrow)").clicked() {
            timeline.step(1);
        }

        let last = timeline.axis.len() - 1;
        let label = timeline.axis.label(timeline.index, timeline.unit);
        let gated = timeline.axis.is_gated(timeline.index);

        // The slider comes first at a fixed width: widgets laid out before it would shift its
        // auto-generated id when they appear (the faulty-frame marker), which drops an active drag.
        ui.spacing_mut().slider_width = (ui.available_width() - TIME_LABEL_WIDTH).max(80.0);
        ui.add(egui::Slider::new(&mut timeline.index, 0..=last).show_value(false));
        ui.label(label);
        if gated {
            ui.label(egui::RichText::new("faulty frame").color(egui::Color32::LIGHT_RED))
                .on_hover_text("quality weight 0: the pipeline judged this frame faulty and excluded it from dating");
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            egui::ComboBox::from_id_salt("time_unit")
                .selected_text(timeline.unit.name())
                .show_ui(ui, |ui| {
                    for unit in TimeUnit::ALL {
                        if timeline.axis.available(unit) {
                            ui.selectable_value(&mut timeline.unit, unit, unit.name());
                        }
                    }
                });
        });
    });
}

/// Colour scale with its end values, for the continuous colour-by modes. Clicking the bar
/// opens a menu of colormaps.
fn legend(ui: &mut egui::Ui, store: &Store, layers: &mut Layers, timeline: Option<&Timeline>) {
    let (lo, hi) = match layers.colour_by {
        ColourBy::TFirst => {
            let last = timeline.map_or(0, |t| t.axis.len().saturating_sub(1));
            ("frame 0".to_string(), format!("frame {last}"))
        }
        ColourBy::Width => match crate::colour::width_range(store) {
            Some((lo, hi)) => (format!("≤ {lo:.1} µm"), format!("≥ {hi:.1} µm")),
            None => {
                ui.label(egui::RichText::new("no widths in this store").weak());
                return;
            }
        },
        _ => return,
    };
    let bar = ui
        .horizontal(|ui| {
            let size = egui::vec2((ui.available_width() - 28.0).clamp(60.0, 160.0), 10.0);
            let bar = colour_bar(ui, layers.scale, size, egui::Sense::click())
                .on_hover_text(format!("{} — click to change the colormap", layers.scale.cmap.name()))
                .on_hover_cursor(egui::CursorIcon::PointingHand);
            // ⇄ is only in the bundled monospace font (Hack), so ask for monospace explicitly
            let icon = egui::RichText::new("⇄").monospace();
            if ui.selectable_label(layers.scale.reversed, icon).on_hover_text("reverse the colormap").clicked() {
                layers.scale.reversed = !layers.scale.reversed;
            }
            bar
        })
        .inner;
    egui::Popup::menu(&bar).show(|ui| {
        for cmap in Cmap::ALL {
            let row = ui.horizontal(|ui| {
                let preview = Scale { cmap, reversed: layers.scale.reversed };
                colour_bar(ui, preview, egui::vec2(90.0, 10.0), egui::Sense::hover());
                ui.selectable_label(layers.scale.cmap == cmap, cmap.name())
            });
            if row.inner.clicked() || row.response.interact(egui::Sense::click()).clicked() {
                layers.scale.cmap = cmap;
                ui.close();
            }
        }
    });
    ui.horizontal(|ui| {
        ui.small(lo);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| ui.small(hi));
    });
}

/// Paints the scale left to right into a new `size` rect.
fn colour_bar(ui: &mut egui::Ui, cmap: Scale, size: egui::Vec2, sense: egui::Sense) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(size, sense);
    const STEPS: usize = 32;
    for i in 0..STEPS {
        let [r, g, b, _] = cmap.sample(i as f32 / (STEPS - 1) as f32);
        let x0 = rect.left() + rect.width() * i as f32 / STEPS as f32;
        let x1 = rect.left() + rect.width() * (i + 1) as f32 / STEPS as f32;
        ui.painter().rect_filled(
            egui::Rect::from_x_y_ranges(x0..=x1, rect.y_range()),
            0.0,
            egui::Color32::from_rgb((r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8),
        );
    }
    response
}

/// One small table per dataset: column names, then the rows about the selected edge.
fn raw_tables(ui: &mut egui::Ui, tables: &[crate::devtools::RawTable]) {
    for t in tables.iter().filter(|t| !t.rows.is_empty()) {
        ui.label(egui::RichText::new(format!("{}  ({} rows)", t.dataset, t.rows.len())).strong().small());
        egui::Grid::new(&t.dataset).striped(true).show(ui, |ui| {
            for c in &t.columns {
                ui.label(egui::RichText::new(c).weak().small());
            }
            ui.end_row();
            for row in &t.rows {
                for v in row {
                    ui.monospace(v);
                }
                ui.end_row();
            }
        });
        ui.add_space(4.0);
    }
}
