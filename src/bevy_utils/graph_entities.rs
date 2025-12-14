use bevy::{
    color::Color,
    math::{Vec2, Vec3},
    prelude::Bundle,
    sprite::Sprite,
    transform::components::Transform,
};

#[derive(Bundle)]
pub struct EntityNode {
    model: Sprite,
    transform: Transform,
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
        }
    }
}
