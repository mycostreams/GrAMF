use crate::{
    bevy_utils::camera_controls::camera_controls_plugin,
    gramf_ui::{ui_graph::spawn_graph, ui_layout::ui_system},
    graphs::stg_graph::SpatioTemporalGraph,
};
use bevy::prelude::*;
use bevy_egui::{EguiPlugin, EguiPrimaryContextPass};

/// Bevy utilities module containing various helper functions and structures to
/// integrate crate structures with Bevy's ECS system.
pub mod bevy_utils;

/// grAMF UI module containing widgets and UI layout definitions.
pub mod gramf_ui;

/// Graphs module containing various graph implementations and related structures.
pub mod graphs;

fn main() {
    App::new()
        // .add_message::<ResetCameraEvent>()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "GrAMF".into(),
                resolution: (1280, 720).into(),
                ..Default::default()
            }),
            ..Default::default()
        }))
        .add_plugins(EguiPlugin::default())
        .add_systems(Startup, setup)
        // .add_systems(Update, camera_controls::controls)
        // .add_systems(Update, update_edge_scale)
        // .add_systems(Update, camera_controls::reset_camera)
        .add_plugins(camera_controls_plugin)
        .add_systems(EguiPrimaryContextPass, ui_system)
        .init_resource::<SpatioTemporalGraph>()
        .run();
}

fn setup(mut commands: Commands, mut stg_graph: ResMut<SpatioTemporalGraph>) {
    // Camera
    commands.spawn(Camera2d);

    *stg_graph = SpatioTemporalGraph::generate_simple();
    spawn_graph(&stg_graph, &mut commands);
}
