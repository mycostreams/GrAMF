use crate::{
    bevy_utils::{camera_controls, graph_entities::EntityNode},
    graphs::{edges::EdgeData, nodes::NodeData, STG_graph::SpatioTemporalGraph},
};
use bevy::prelude::*;
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

    let nodes = [
        NodeData {
            pos: (10.0, 0.0, 0.0).into(),
            id: 0,
        },
        NodeData {
            pos: (0.0, 10.0, 0.0).into(),
            id: 1,
        },
        NodeData {
            pos: (10.0, 10.0, 0.0).into(),
            id: 2,
        },
    ];

    for node in nodes {
        stg_graph.graph.add_node(node);
    }

    stg_graph.graph.add_edge(
        nodes[0].into(),
        nodes[1].into(),
        EdgeData {
            width: 0.5,
            node_poss: (nodes[0].pos, nodes[1].pos),
        },
    );

    for node in stg_graph.graph.raw_nodes() {
        _spawn_node(node.weight, &mut commands);
        commands.spawn((
            Mesh2d(meshes.add(Segment2d::new(
                [nodes[0].pos.x, nodes[0].pos.y].into(),
                [nodes[1].pos.x, nodes[1].pos.y].into(),
            ))),
            MeshMaterial2d(materials.add(Color::WHITE)),
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

fn _spawn_node(
    node: NodeData,
    commands: &mut Commands,
    // meshes: ResMut<Assets<Mesh>>,
    // materials: ResMut<Assets<ColorMaterial>>,
) {
    commands
        .spawn((EntityNode::new(node.pos), Pickable::default()))
        .observe(recolor_on::<Pointer<Over>>(Color::srgb(0.0, 1.0, 1.0)))
        .observe(recolor_on::<Pointer<Out>>(Color::BLACK))
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
