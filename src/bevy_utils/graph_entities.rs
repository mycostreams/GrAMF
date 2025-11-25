use bevy::{
    color::Color,
    math::{primitives::Segment2d, Vec2, Vec3},
    prelude::Bundle,
    sprite::Sprite,
    sprite_render::ColorMaterial,
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

// #[derive(Bundle)]
// pub struct EntityEdge {
//     model: Segment2d,
//     node_positions: (Vec3, Vec3),
//     color: ColorMaterial
// }
// impl EntityEdge {
//     pub fn new(pos_a: Vec3, pos_b: Vec3) -> Self {
//         EntityEdge {
//             model: Segment2d {
//                 vertices: [[pos_a.x, pos_a.y].into(), [pos_b.x, pos_b.y].into()],
//             },
//             node_positions: (pos_a, pos_b),
//             color: ColorMaterial::from_color(Color::WHITE)
//         }
//     }
// }
