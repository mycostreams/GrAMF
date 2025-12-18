use bevy::{ecs::component::Component, math::Vec3};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::graphs::types::TimeSeries;

#[derive(Debug, Clone, Copy)]
pub struct EdgeData {
    pub node_poss: (Vec3, Vec3),
}

impl EdgeData {
    pub fn length(&self) -> f32 {
        (self.node_poss.1 - self.node_poss.0).length()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Component)]
pub struct EdgeProperties {
    pub diameter: Option<f64>,
    #[serde(flatten)]
    pub other: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone)]
pub struct Edge {
    pub source: String,
    pub target: String,
    pub properties: TimeSeries<EdgeProperties>,
}

#[test]
fn test_edge_data_length() {
    let edge = EdgeData {
        node_poss: (Vec3::ZERO, Vec3::new(3.0, 4.0, 0.0)),
    };
    assert_eq!(edge.length(), 5.0);
}
