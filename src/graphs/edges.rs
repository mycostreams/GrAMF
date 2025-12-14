use bevy::math::Vec3;

#[derive(Debug, Clone, Copy)]
pub(crate) struct EdgeData {
    pub(crate) node_poss: (Vec3, Vec3),
}

impl EdgeData {
    fn length(&self) -> f32 {
        (self.node_poss.1 - self.node_poss.0).length()
    }
}

#[test]
fn test_edge_data_length() {
    let edge = EdgeData {
        node_poss: (Vec3::ZERO, Vec3::new(3.0, 4.0, 0.0)),
    };
    assert_eq!(edge.length(), 5.0);
}
