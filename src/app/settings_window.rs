//! The Settings window: author, plate registry folder, registration session gap.

use crate::settings::Settings;

#[derive(Default)]
pub struct SettingsWindow {
    /// Edited copy while the window is open.
    draft: Option<Settings>,
}

impl SettingsWindow {
    pub fn open(&mut self, current: &Settings) {
        self.draft = Some(current.clone());
    }

    pub fn is_open(&self) -> bool {
        self.draft.is_some()
    }
}

/// Shows the window while open; returns the new settings when the user saves.
/// `author_locked` keeps the author fixed while annotations are unsaved.
pub fn show(ctx: &egui::Context, w: &mut SettingsWindow, author_locked: bool) -> Option<Settings> {
    let draft = w.draft.as_mut()?;
    let mut saved = None;
    let mut close = false;
    egui::Window::new("Settings").collapsible(false).resizable(false).show(ctx, |ui| {
        egui::Grid::new("settings").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
            ui.label("Author");
            ui.vertical(|ui| {
                ui.add_enabled(!author_locked, egui::TextEdit::singleline(&mut draft.author).desired_width(180.0));
                let note = if author_locked {
                    "save your annotations before switching author"
                } else {
                    "your annotations are saved as annotations.<author>.h5"
                };
                ui.label(egui::RichText::new(note).weak().small());
            });
            ui.end_row();

            ui.label("Plate registry");
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    let mut text = draft.registry_dir.display().to_string();
                    if ui.add(egui::TextEdit::singleline(&mut text).desired_width(260.0)).changed() {
                        draft.registry_dir = text.into();
                    }
                    if ui.button("Browse…").clicked() {
                        if let Some(dir) = rfd::FileDialog::new().set_directory(&draft.registry_dir).pick_folder() {
                            draft.registry_dir = dir;
                        }
                    }
                });
                let hint = if std::env::var_os("GRAMF_REGISTRY").is_some() {
                    "overridden by GRAMF_REGISTRY in the environment"
                } else {
                    "the AMF-PlateList folder (NOW_ / OLD_Experiments xlsx)"
                };
                ui.label(egui::RichText::new(hint).weak().small());
            });
            ui.end_row();

            ui.label("Session gap");
            ui.vertical(|ui| {
                ui.add(egui::DragValue::new(&mut draft.session_gap_minutes).range(1.0..=1440.0).suffix(" min"));
                ui.label(egui::RichText::new("video runs further apart start a new registration session").weak().small());
            });
            ui.end_row();
        });
        ui.separator();
        ui.horizontal(|ui| {
            if ui.button("Save").clicked() {
                draft.author = crate::annotations::sanitize_author(&draft.author);
                if draft.author.is_empty() {
                    draft.author = Settings::default().author;
                }
                saved = Some(draft.clone());
                close = true;
            }
            if ui.button("Cancel").clicked() {
                close = true;
            }
        });
    });
    if close {
        w.draft = None;
    }
    saved
}
