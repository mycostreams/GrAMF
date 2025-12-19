use bevy::{
    color::Color,
    ecs::component::Component,
    math::{Vec2, Vec3},
    prelude::Bundle,
    sprite::Sprite,
    transform::components::Transform,
};

use crate::bevy_utils::resource_config::EDGE_WIDTH_SCALE_VISIBLE;

#[derive(Component)]
pub(crate) struct NodeTag;
#[derive(Component)]
pub(crate) struct EdgeTag;

#[derive(Bundle)]
pub struct EntityNode {
    model: Sprite,
    transform: Transform,
    graph_type: NodeTag,
}

#[derive(Bundle)]
pub struct EntityEdge {
    model: Sprite,
    transform: Transform,
    graph_type: EdgeTag,
}

impl EntityNode {
    pub fn new(pos: Vec3) -> Self {
        EntityNode {
            model: Sprite {
                color: Color::WHITE,
                custom_size: Some(Vec2::splat(2.0)),
                ..Default::default()
            },
            transform: Transform::from_translation(pos),
            graph_type: NodeTag,
        }
    }
}

impl EntityEdge {
    pub fn new(pos: Vec3, length: f32, angle: f32) -> Self {
        EntityEdge {
            model: Sprite {
                color: Color::WHITE,
                custom_size: Some(Vec2::new(length, EDGE_WIDTH_SCALE_VISIBLE)),
                ..Default::default()
            },
            transform: Transform::from_translation(pos)
                .with_rotation(bevy::math::Quat::from_rotation_z(angle)),
            graph_type: EdgeTag,
        }
    }
    pub(crate) fn from_edge_data(edge_data: &crate::graph_model::edges::EdgeData) -> Self {
        let midpoint = (edge_data.node_poss.0 + edge_data.node_poss.1) / 2.0;
        let direction = (edge_data.node_poss.1 - edge_data.node_poss.0).normalize();
        let distance = edge_data.node_poss.0.distance(edge_data.node_poss.1);
        let angle = direction.y.atan2(direction.x);

        EntityEdge::new(midpoint, distance, angle)
    }
}
