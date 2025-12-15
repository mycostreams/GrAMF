use bevy::color::Color;
use bevy::ecs::message::MessageWriter;
use bevy::{
    ecs::{event::EntityEvent, observer::On, system::Query},
    prelude::Result,
};
use bevy::{reflect::Reflect, sprite::Sprite};
use bevy_egui::{egui, EguiContexts};

use crate::bevy_utils::camera_controls::ResetCameraEvent;

/// UI layout system that defines the structure and behavior of the UI components.
pub(crate) fn ui_system(
    mut contexts: EguiContexts,
    mut reset_event: MessageWriter<ResetCameraEvent>,
) -> Result {
    let ctx = contexts.ctx_mut()?;

    egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
        // The top panel is often a good place for a menu bar:
        egui::MenuBar::new().ui(ui, |ui| {
            egui::containers::menu::MenuButton::new("File").ui(ui, |ui| {
                if ui.button("Quit").clicked() {
                    std::process::exit(0);
                }
            });
            egui::containers::menu::MenuButton::new("View").ui(ui, |ui| {
                if ui.button("Reset Camera").clicked() {
                    reset_event.write(ResetCameraEvent);
                }
            });
        });
    });
    Ok(())
}

/// Recolor the sprite of an entity when an event of type E occurs on it.
pub(crate) fn recolor_sprite<E: EntityEvent + Clone + Reflect>(
    color: Color,
) -> impl Fn(On<E>, Query<&mut Sprite>) {
    move |ev, mut sprites| {
        let Ok(mut sprite) = sprites.get_mut(ev.event_target()) else {
            return;
        };
        sprite.color = color;
    }
}
