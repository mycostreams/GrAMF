use bevy::math::Vec3;
use petgraph::graph::NodeIndex;

#[derive(Debug, Clone, Default, Copy)]

pub(crate) struct NodeData {
    pub(crate) pos: Vec3,
    pub(crate) id: usize,
}

impl From<NodeData> for NodeIndex {
    fn from(value: NodeData) -> Self {
        NodeIndex::new(value.id)
    }
}


#[test]
fn test_node_data_conversion() {
    let node = NodeData { pos: Vec3::ZERO, id: 5 };
    let index: NodeIndex = node.into();
    assert_eq!(index.index(), 5);
}