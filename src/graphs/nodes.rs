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
