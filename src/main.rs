use bevy::{
    input::mouse::{AccumulatedMouseScroll, MouseMotion, MouseWheel},
    math::ops::powf,
    prelude::*,
    transform,
};
use rand::Rng;
use crate::bevy_utils::camera_controls;
pub mod bevy_utils;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "GrAMF".into(),
                resolution: (1280, 720).into(),
                ..Default::default()
            }),
            ..Default::default()
        }))
        .add_systems(Startup, setup)
        .add_systems(Update, camera_controls::controls)
        .run();
}

fn setup(mut commands: Commands) {
    // Camera
    commands.spawn(Camera2d);

    // Node material
    let mut rng = rand::rng();

    // Spawn 10k nodes (Bevy will batch these into instanced draw calls)
    for _ in 0..10_000 {
        let x = rng.random_range(-500.0..500.0);
        let y = rng.random_range(-500.0..500.0);

        commands.spawn((
            Sprite {
                color: Color::WHITE,
                custom_size: Some(Vec2::splat(2.0)),
                ..Default::default()
            },
            Transform::from_xyz(x, y, 0.0),
        ));
    }
}
