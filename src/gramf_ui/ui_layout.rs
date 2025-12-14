use bevy::color::Color;
use bevy::{
    ecs::{event::EntityEvent, observer::On, system::Query},
    prelude::Result,
};
use bevy::{reflect::Reflect, sprite::Sprite};
use bevy_egui::{egui, EguiContexts};

pub(crate) fn ui_system(mut contexts: EguiContexts) -> Result {
    let ctx = contexts.ctx_mut()?;

    egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
        // The top panel is often a good place for a menu bar:
        egui::MenuBar::new().ui(ui, |ui| {
            egui::containers::menu::MenuButton::new("File").ui(ui, |ui| {
                if ui.button("Quit").clicked() {
                    std::process::exit(0);
                }
            });
        });
    });
    Ok(())
}

/// Recolor the sprite of an entity when an event of type E occurs on it.
pub fn recolor_on<E: EntityEvent + Clone + Reflect>(
    color: Color,
) -> impl Fn(On<E>, Query<&mut Sprite>) {
    move |ev, mut sprites| {
        let Ok(mut sprite) = sprites.get_mut(ev.event_target()) else {
            return;
        };
        sprite.color = color;
    }
}
