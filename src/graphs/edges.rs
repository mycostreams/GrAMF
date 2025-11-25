use bevy::math::Vec3;

#[derive(Debug, Clone)]
pub(crate) struct EdgeData {
    width: f32,
    node_poss: (Vec3, Vec3)
}

impl EdgeData {
    fn length(&self) -> f32 {
        (self.node_poss.1 - self.node_poss.0).length()
    }
}