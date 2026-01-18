use bevy::{ecs::component::Component, math::Vec3, ui::Node};
use serde::{Deserialize, Serialize};
use serde_json::Map;

use crate::graph_model::{nodes::NodeID, types::TimeSeries};

pub type EdgeID = i64;
pub type EdgeClusterID = i64;

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
    pub source: NodeID,
    pub target: NodeID,
    pub id: EdgeID,
}

impl EdgeLight {
    pub fn length(&self) -> f32 {
        self.node_poss.0.distance(self.node_poss.1)
    }
}

#[derive(Clone, Component, Debug)]
pub struct EdgeFull {
    pub source: NodeID,
    pub target: NodeID,
    pub id: EdgeID,
    pub edge_cluster_id: EdgeClusterID,
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
            id: 0,
            edge_cluster_id: 0,
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
