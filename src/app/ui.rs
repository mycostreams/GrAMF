use crate::colour::ColourBy;
use crate::store::Store;
use crate::time::{TimeAxis, TimeUnit};

/// Room right of the slider for the time label, gated marker and unit picker (points).
const TIME_LABEL_WIDTH: f32 = 420.0;

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
}

pub struct Layers {
    pub colour_by: ColourBy,
    pub node_ids: bool,
    pub hyperedge_ids: bool,
}

impl Default for Layers {
    fn default() -> Self {
        Self {
            colour_by: ColourBy::Uniform,
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
}

/// What the user asked for this frame; acted on outside the egui closure.
#[derive(Default)]
pub struct UiActions {
    pub open: bool,
    pub colour_changed: bool,
    pub select: Option<Selection>,
    /// Area left over between the panels, in points: where the map is drawn.
    pub map_rect: Option<egui::Rect>,
}

pub fn build_ui(ui: &mut egui::Ui, frame: FrameState) -> UiActions {
    let FrameState { title, status, timeline, layers, store, selection, labels } = frame;
    let mut actions = UiActions::default();
    egui::Panel::top("top").show_inside(ui, |ui| {
        ui.horizontal(|ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Open store…").clicked() {
                        actions.open = true;
                        ui.close();
                    }
                    if ui.button("Save").clicked() {
                        ui.close();
                    }
                    ui.separator();
                    if ui.button("Import").clicked() {
                        ui.close();
                    }
                });

                ui.menu_button("Edit", |ui| {
                    if ui.button("Clear").clicked() {
                        ui.close();
                    }
                    if ui.button("Reset").clicked() {
                        ui.close();
                    }
                });

                ui.separator();

                ui.label(egui::RichText::new(title).color(egui::Color32::LIGHT_GREEN));

                ui.separator();
                ui.label(status);
            });

            ui.with_layout(egui::Layout::right_to_left(egui::Align::RIGHT), |ui| {
                ui.add_space(10.0);
                ui.label(egui::RichText::new("v0.1.0").color(egui::Color32::ORANGE));
                ui.separator();
            });
        });
    });

    let timeline = timeline.map(|t| {
        egui::Panel::bottom("timeline").show_inside(ui, |ui| timeline_bar(ui, t));
        &*t
    });

    egui::Panel::left("layers").show_inside(ui, |ui| {
        ui.heading("Layers");
        let before = layers.colour_by;
        egui::ComboBox::from_label("colour")
            .selected_text(layers.colour_by.name())
            .show_ui(ui, |ui| {
                for by in ColourBy::ALL {
                    ui.selectable_value(&mut layers.colour_by, by, by.name());
                }
            });
        actions.colour_changed = layers.colour_by != before;
        if let Some(store) = store {
            legend(ui, store, layers.colour_by, timeline);
        }
        ui.checkbox(&mut layers.node_ids, "Node ids");
        ui.checkbox(&mut layers.hyperedge_ids, "Hyperedge ids");
        if (layers.node_ids || layers.hyperedge_ids) && labels.is_none() {
            ui.label(egui::RichText::new("zoom in to see ids").weak());
        }
    });

    egui::Panel::right("right").show_inside(ui, |ui| {
        ui.heading("Inspector");
        ui.separator();
        match (store, selection) {
            (Some(store), Some(sel)) => actions.select = inspector(ui, store, sel, timeline),
            _ => {
                ui.label(egui::RichText::new("Click an edge or node").weak());
            }
        }
    });

    let map = ui.available_rect_before_wrap();
    if let Some(labels) = labels {
        let painter = ui.painter_at(map);
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

/// Property rows for the selection; returns a new selection if the user asked for one.
fn inspector(ui: &mut egui::Ui, store: &Store, sel: Selection, timeline: Option<&Timeline>) -> Option<Selection> {
    let mut select = None;
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
                row(ui, "width", match store.width[e] {
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
    select
}

fn timeline_bar(ui: &mut egui::Ui, timeline: &mut Timeline) {
    ui.horizontal(|ui| {
        if ui.button("◀").on_hover_text("Previous frame (←)").clicked() {
            timeline.step(-1);
        }
        let play = if timeline.playing { "⏸" } else { "⏵" };
        if ui.button(play).on_hover_text("Play (Space)").clicked() {
            timeline.toggle_play();
        }
        if ui.button("▶").on_hover_text("Next frame (→)").clicked() {
            timeline.step(1);
        }

        let last = timeline.axis.len() - 1;
        let label = timeline.axis.label(timeline.index, timeline.unit);
        let gated = timeline.axis.is_gated(timeline.index);

        // The slider comes first at a fixed width: widgets laid out before it would shift its
        // auto-generated id when they appear (the gated marker), which drops an active drag.
        ui.spacing_mut().slider_width = (ui.available_width() - TIME_LABEL_WIDTH).max(80.0);
        ui.add(egui::Slider::new(&mut timeline.index, 0..=last).show_value(false));
        ui.label(label);
        if gated {
            ui.label(egui::RichText::new("gated").color(egui::Color32::LIGHT_RED))
                .on_hover_text("quality weight 0: excluded from dating");
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

/// Colour scale with its end values, for the continuous colour-by modes.
fn legend(ui: &mut egui::Ui, store: &Store, by: ColourBy, timeline: Option<&Timeline>) {
    let (lo, hi) = match by {
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
    let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width().min(160.0), 10.0), egui::Sense::hover());
    const STEPS: usize = 32;
    for i in 0..STEPS {
        let [r, g, b, _] = crate::colour::viridis(i as f32 / (STEPS - 1) as f32);
        let x0 = rect.left() + rect.width() * i as f32 / STEPS as f32;
        let x1 = rect.left() + rect.width() * (i + 1) as f32 / STEPS as f32;
        ui.painter().rect_filled(
            egui::Rect::from_x_y_ranges(x0..=x1, rect.y_range()),
            0.0,
            egui::Color32::from_rgb((r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8),
        );
    }
    ui.horizontal(|ui| {
        ui.small(lo);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| ui.small(hi));
    });
}
