use bevy::math::Vec3;

/// ## Edge Data
/// Represents the data associated with an edge in the spatio-temporal graph.

#[derive(Debug, Clone, Copy)]
pub struct EdgeData {
    pub node_poss: (Vec3, Vec3),
    pub source: usize,
    pub target: usize,
    pub id: usize,
}

impl EdgeData {
    pub fn length(&self) -> f32 {
        self.node_poss.0.distance(self.node_poss.1)
    }
}

#[test]
fn test_edge_data_length() {
    let edge = EdgeData {
        node_poss: (Vec3::ZERO, Vec3::new(3.0, 4.0, 0.0)),
        source: 1,
        target: 2,
        id: 0,
    };
    assert_eq!(edge.length(), 5.0);
}
