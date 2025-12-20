use bevy::{ecs::component::Component, math::Vec3};

use crate::graph_model::types::TimeSeries;

#[derive(Clone, Copy)]
pub enum EdgeProgress {
    NotStarted,
    Growing(f32), // 0.0 to 1.0
    Complete,
    Septating(f32), // 1.0 to 0.0
    Septated,
}

/// ## Edge Data
/// Represents the data associated with an edge in the spatio-temporal graph.

#[derive(Debug, Clone, Copy)]
pub struct EdgeLight {
    pub node_poss: (Vec3, Vec3),
    pub source: usize,
    pub target: usize,
    pub id: usize,
}

impl EdgeLight {
    pub fn length(&self) -> f32 {
        self.node_poss.0.distance(self.node_poss.1)
    }
}

#[derive(Clone, Component)]
pub struct EdgeFull {
    pub node_poss: (Vec3, Vec3),
    pub source: usize,
    pub target: usize,
    pub id: usize,
    pub edge_cluster_id: usize,
    pub temporal_props: TimeSeries<EdgeTemporals>,
}

#[derive(Clone)]
pub struct EdgeTemporals {
    pub width: f32,
    pub progress: EdgeProgress,
    pub other_data: serde_json::Map<String, serde_json::Value>,
}

impl EdgeFull {
    pub fn length(&self) -> f32 {
        self.node_poss.0.distance(self.node_poss.1)
    }
}

#[test]
fn test_edge_data_length() {
    let edge = EdgeLight {
        node_poss: (Vec3::ZERO, Vec3::new(3.0, 4.0, 0.0)),
        source: 1,
        target: 2,
        id: 0,
    };
    assert_eq!(edge.length(), 5.0);
}
