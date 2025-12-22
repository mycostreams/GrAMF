use bevy::{ecs::component::Component, math::Vec3};
use serde::{Deserialize, Serialize};
use serde_json::Map;

use crate::graph_model::types::TimeSeries;

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
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

#[derive(Clone, Component, Debug)]
pub struct EdgeFull {
    pub source: i64,
    pub target: i64,
    pub id: i64,
    pub edge_cluster_id: i64,
    pub temporal_props: TimeSeries<EdgeTemporals>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EdgeTemporals {
    pub width: f32,
    pub progress: EdgeProgress,
    pub other_data: serde_json::Map<String, serde_json::Value>,
}

impl EdgeFull {
    pub fn new() -> Self {
        EdgeFull {
            source: 0,
            target: 1,
            id: 1,
            edge_cluster_id: 3,
            temporal_props: TimeSeries {
                timestamps: vec![EdgeTemporals::new()],
            },
        }
    }
}

impl EdgeTemporals {
    pub fn new() -> Self {
        EdgeTemporals {
            width: 0.0,
            progress: EdgeProgress::NotStarted,
            other_data: Map::new(),
        }
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
