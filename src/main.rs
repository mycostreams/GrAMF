use crate::bevy_utils::camera_controls;
use bevy::prelude::*;
use rand::Rng;
pub mod bevy_utils;
pub mod graphs;

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

fn setup(mut commands: Commands, 
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,

) {
    // Camera
    commands.spawn(Camera2d);

    let spatiograph = graphs::return_simple_graph();
    println!("{:?}", spatiograph);

    for node in spatiograph.graph.raw_nodes() {
        commands.spawn((
            Sprite{
                color: Color::WHITE,
                custom_size: Some(Vec2::splat(2.0)),
                ..Default::default()
            },
            Transform::from_translation(node.weight.pos)
        ));
    }

    // // Node material
    // let mut rng = rand::rng();

    // // Spawn 10k nodes (Bevy will batch these into instanced draw calls)
    // for _ in 0..10_000 {
    //     let x = rng.random_range(-500.0..500.0);
    //     let y = rng.random_range(-500.0..500.0);

    //     commands.spawn((
    //         Sprite {
    //             color: Color::WHITE,
    //             custom_size: Some(Vec2::splat(2.0)),
    //             ..Default::default()
    //         },
    //         Transform::from_xyz(x, y, 0.0),
    //     ));
    // }
}
