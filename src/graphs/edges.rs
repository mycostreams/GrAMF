use bevy::math::Vec3;

#[derive(Debug, Clone)]
pub(crate) struct EdgeData {
    pub(crate) width: f32,
    pub(crate) node_poss: (Vec3, Vec3),
}

impl EdgeData {
    fn length(&self) -> f32 {
        (self.node_poss.1 - self.node_poss.0).length()
    }
}
