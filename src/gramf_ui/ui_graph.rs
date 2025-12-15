use bevy::{
    color::Color,
    ecs::system::Commands,
    picking::{
        events::{Out, Over, Pointer, Press, Release},
        Pickable,
    },
};

use crate::{
    bevy_utils::graph_entities::{EntityEdge, EntityNode},
    gramf_ui::ui_layout::recolor_sprite,
    graphs::{edges::EdgeData, nodes::NodeData, stg_graph::SpatioTemporalGraph},
};

/// Spawn the entire graph: nodes and edges.
pub(crate) fn spawn_graph(stg_graph: &SpatioTemporalGraph, commands: &mut Commands) {
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
        .spawn((EntityNode::new(node.pos), Pickable::default()))
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
        .spawn((EntityEdge::from_edge_data(&edge_data), Pickable::default()))
        .observe(recolor_sprite::<Pointer<Over>>(Color::srgb(1.0, 0.0, 1.0)))
        .observe(recolor_sprite::<Pointer<Out>>(Color::WHITE))
        .observe(recolor_sprite::<Pointer<Press>>(Color::srgb(1.0, 1.0, 0.0)))
        .observe(recolor_sprite::<Pointer<Release>>(Color::srgb(
            1.0, 0.0, 1.0,
        )));
}
