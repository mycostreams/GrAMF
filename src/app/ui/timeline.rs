//! The time axis: playback state and the timeline bar under the map.

use super::*;

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

pub(super) fn timeline_bar(ui: &mut egui::Ui, timeline: &mut Timeline) {
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
