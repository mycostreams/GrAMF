use crate::{
    bevy_utils::{camera_controls, graph_entities::EntityNode},
    gramf_ui::ui_layout::ui_system,
    graphs::{edges::EdgeData, nodes::NodeData, stg_graph::SpatioTemporalGraph},
};
use bevy::{
    prelude::*,
    remote::{http::RemoteHttpPlugin, RemotePlugin},
};
use bevy_egui::{EguiPlugin, EguiPrimaryContextPass};
pub mod bevy_utils;
pub mod gramf_ui;

/// Graphs module containing various graph implementations and related structures.
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
        .add_plugins(EguiPlugin::default())
        .add_systems(Startup, setup)
        .add_plugins(RemotePlugin::default()) // Core remote protocol
        .add_plugins(RemoteHttpPlugin::default()) // Enable HTTP transport
        .add_systems(Update, camera_controls::controls)
        .add_systems(EguiPrimaryContextPass, ui_system)
        .init_resource::<SpatioTemporalGraph>()
        .run();
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut stg_graph: ResMut<SpatioTemporalGraph>,
) {
    // Camera
    commands.spawn(Camera2d);

    *stg_graph = SpatioTemporalGraph::generate_simple();
    spawn_graph(&stg_graph, &mut commands, &mut meshes, &mut materials);
}

fn spawn_graph(
    stg_graph: &SpatioTemporalGraph,
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<ColorMaterial>>,
) {
    for node in stg_graph.graph.raw_nodes() {
        spawn_node(node.weight, commands);
    }
    for edge in stg_graph.graph.raw_edges() {
        commands.spawn((
            Mesh2d(
                meshes.add(Segment2d::new(
                    [
                        stg_graph.graph[edge.source()].pos.x,
                        stg_graph.graph[edge.source()].pos.y,
                    ]
                    .into(),
                    [
                        stg_graph.graph[edge.target()].pos.x,
                        stg_graph.graph[edge.target()].pos.y,
                    ]
                    .into(),
                )),
            ),
            MeshMaterial2d(materials.add(Color::WHITE)),
        ));
    }
}

fn spawn_node(
    node: NodeData,
    commands: &mut Commands,
    // meshes: ResMut<Assets<Mesh>>,
    // materials: ResMut<Assets<ColorMaterial>>,
) {
    commands
        .spawn((EntityNode::new(node.pos), Pickable::default()))
        .observe(recolor_on::<Pointer<Over>>(Color::srgb(0.0, 1.0, 1.0)))
        .observe(recolor_on::<Pointer<Out>>(Color::WHITE))
        .observe(recolor_on::<Pointer<Press>>(Color::srgb(1.0, 1.0, 0.0)))
        .observe(recolor_on::<Pointer<Release>>(Color::srgb(0.0, 1.0, 1.0)));
}

fn recolor_on<E: EntityEvent + Clone + Reflect>(
    color: Color,
) -> impl Fn(On<E>, Query<&mut Sprite>) {
    move |ev, mut sprites| {
        let Ok(mut sprite) = sprites.get_mut(ev.event_target()) else {
            return;
        };
        sprite.color = color;
    }
}
