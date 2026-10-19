use crate::time::{TimeAxis, TimeUnit};

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

/// What the user asked for this frame; acted on outside the egui closure.
#[derive(Default)]
pub struct UiActions {
    pub open: bool,
    /// Area left over between the panels, in points: where the map is drawn.
    pub map_rect: Option<egui::Rect>,
}

pub fn build_ui(
    ui: &mut egui::Ui,
    title: &str,
    status: &str,
    timeline: Option<&mut Timeline>,
) -> UiActions {
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

    if let Some(timeline) = timeline {
        egui::Panel::bottom("timeline").show_inside(ui, |ui| timeline_bar(ui, timeline));
    }

    // egui::Panel::left("left").show_inside(ui, |ui| {
    //     ui.heading("Scene Tree");
    // });

    egui::Panel::right("right").show_inside(ui, |ui| {
        ui.heading("Inspector");
    });

    actions.map_rect = Some(ui.available_rect_before_wrap());
    actions
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

        // Label and unit picker sit on the right; the slider takes the remaining width.
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
            if gated {
                ui.label(egui::RichText::new("gated frame").color(egui::Color32::LIGHT_RED))
                    .on_hover_text("quality weight 0: excluded from dating");
            }
            ui.label(label);
            ui.spacing_mut().slider_width = ui.available_width() - 8.0;
            ui.add(egui::Slider::new(&mut timeline.index, 0..=last).show_value(false));
        });
    });
}
