use bevy::math::Vec3;
use petgraph::graph::NodeIndex;
use serde::{Deserialize, Serialize};

pub type NodeID = i64;

#[derive(Debug, Clone, Default, Copy, Serialize, Deserialize)]
pub struct NodeData {
    pub pos: Vec3,
    pub id: NodeID,
}

impl From<NodeData> for NodeIndex {
    fn from(value: NodeData) -> Self {
        NodeIndex::new(value.id.try_into().unwrap())
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
