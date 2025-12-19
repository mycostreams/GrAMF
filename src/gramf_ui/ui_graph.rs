use bevy::{
    color::Color,
    ecs::{name::Name, system::Commands},
    picking::{
        Pickable,
        events::{Out, Over, Pointer, Press, Release},
    },
};

use crate::{
    bevy_utils::graph_entities::{EntityEdge, EntityNode},
    gramf_ui::ui_layout::recolor_sprite,
    graph_model::{edges::EdgeData, nodes::NodeData, stg_graphs::SnapshotGraph},
};

/// Spawn the entire graph: nodes and edges.
pub(crate) fn spawn_graph(stg_graph: &SnapshotGraph, commands: &mut Commands) {
    for node in stg_graph.graph.raw_nodes() {
        spawn_node(node.weight, commands);
    }
    for edge in stg_graph.graph.raw_edges() {
        spawn_edge(edge.weight, commands);
    }
}

/// Spawn a node at its position with pickable and recoloring behavior.
fn spawn_node(node: NodeData, commands: &mut Commands) {
    commands
        .spawn((
            Name::new("Node"),
            EntityNode::new(node.pos),
            Pickable::default(),
        ))
        .observe(recolor_sprite::<Pointer<Over>>(Color::srgb(0.0, 1.0, 1.0)))
        .observe(recolor_sprite::<Pointer<Out>>(Color::WHITE))
        .observe(recolor_sprite::<Pointer<Press>>(Color::srgb(1.0, 1.0, 0.0)))
        .observe(recolor_sprite::<Pointer<Release>>(Color::srgb(
            0.0, 1.0, 1.0,
        )));
}

/// Spawn an edge between two positions as a rectangle mesh.
fn spawn_edge(edge_data: EdgeData, commands: &mut Commands) {
    commands
        .spawn((
            Name::new("Edge"),
            EntityEdge::from_edge_data(&edge_data),
            Pickable::default(),
        ))
        .observe(recolor_sprite::<Pointer<Over>>(Color::srgb(1.0, 0.0, 1.0)))
        .observe(recolor_sprite::<Pointer<Out>>(Color::WHITE))
        .observe(recolor_sprite::<Pointer<Press>>(Color::srgb(1.0, 1.0, 0.0)))
        .observe(recolor_sprite::<Pointer<Release>>(Color::srgb(
            1.0, 0.0, 1.0,
        )));
}
