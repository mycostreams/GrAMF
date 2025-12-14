use bevy::{
    asset::Assets,
    camera::Projection,
    color::Color,
    ecs::system::{Commands, Query, ResMut, Single},
    math::{primitives::Rectangle, Quat, Vec3},
    mesh::{Mesh, Mesh2d},
    picking::{
        events::{Out, Over, Pointer, Press, Release},
        Pickable,
    },
    sprite_render::{ColorMaterial, MeshMaterial2d},
    transform::components::Transform,
};

use crate::{
    bevy_utils::graph_entities::{EntityNode, UiEdge},
    gramf_ui::ui_layout::recolor_on,
    graphs::{nodes::NodeData, stg_graph::SpatioTemporalGraph},
};


/// Spawn the entire graph: nodes and edges.
pub(crate) fn spawn_graph(
    stg_graph: &SpatioTemporalGraph,
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<ColorMaterial>>,
) {
    for node in stg_graph.graph.raw_nodes() {
        spawn_node(node.weight, commands);
    }
    for edge in stg_graph.graph.raw_edges() {
        spawn_edge(
            stg_graph.graph[edge.source()].pos,
            stg_graph.graph[edge.target()].pos,
            commands,
            meshes,
            materials,
        );
    }
}

/// Spawn a node at its position with pickable and recoloring behavior.
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

/// Spawn an edge between two positions as a rectangle mesh.
fn spawn_edge(
    start_pos: Vec3,
    end_pos: Vec3,
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<ColorMaterial>>,
) {
    let edge_width = 2.0;
    let midpoint = (start_pos + end_pos) / 2.0;
    let direction = (end_pos - start_pos).normalize();
    let distance = start_pos.distance(end_pos);
    let angle = direction.y.atan2(direction.x);

    commands.spawn((
        Mesh2d(meshes.add(Rectangle::new(distance, edge_width))),
        MeshMaterial2d(materials.add(Color::WHITE)),
        Transform::from_translation(midpoint).with_rotation(Quat::from_rotation_z(angle)),
        UiEdge::new(edge_width),
    ));
}

/// Update the scale of edges based on the camera's zoom level to maintain consistent visual thickness.
pub(crate) fn update_edge_scale(
    camera_query: Single<&Projection>,
    mut edge_query: Query<(&mut Transform, &UiEdge)>,
) {
    let Projection::Orthographic(proj) = &*camera_query else {
        return;
    };

    for (mut transform, edge) in &mut edge_query {
        transform.scale.y = edge.base_width * proj.scale;
    }
}