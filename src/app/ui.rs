/// What the user asked for this frame; acted on outside the egui closure.
#[derive(Default)]
pub struct UiActions {
    pub open: bool,
    /// Area left over between the panels, in points: where the map is drawn.
    pub map_rect: Option<egui::Rect>,
}

pub fn build_ui(ui: &mut egui::Ui, title: &str, status: &str) -> UiActions {
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

    // egui::Panel::left("left").show_inside(ui, |ui| {
    //     ui.heading("Scene Tree");
    // });

    egui::Panel::right("right").show_inside(ui, |ui| {
        ui.heading("Inspector");
    });

    // egui::Panel::bottom("Console").show_inside(ui, |ui| {
    //     ui.heading("Console");
    // });

    actions.map_rect = Some(ui.available_rect_before_wrap());
    actions
}
