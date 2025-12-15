use bevy::math::Vec3;
use petgraph::graph::NodeIndex;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Copy, Serialize, Deserialize)]
pub(crate) struct NodeData {
    pub(crate) pos: Vec3,
    pub(crate) id: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    pub id: String,
    pub coordinates: (f64, f64),
}

impl From<NodeData> for NodeIndex {
    fn from(value: NodeData) -> Self {
        NodeIndex::new(value.id)
    }
}

#[test]
fn test_node_data_conversion() {
    let node = NodeData {
        pos: Vec3::ZERO,
        id: 5,
    };
    let index: NodeIndex = node.into();
    assert_eq!(index.index(), 5);
}
