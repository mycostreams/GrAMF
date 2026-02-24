use crate::graph::{
    r_tree::{edge::SpatialEdge, node::SpatialNode},
    topology::{EdgeId, GraphTopology, NodeId},
};
use rstar::RTree;

pub struct SpatialIndex {
    pub nodes: RTree<SpatialNode>,
    pub edges: RTree<SpatialEdge>,
}

impl SpatialIndex {
    pub fn new() -> Self {
        Self {
            nodes: RTree::new(),
            edges: RTree::new(),
        }
    }

    pub fn from_topology(topology: GraphTopology) -> Self {
        let mut new_index = Self::new();
        for node in topology.graph.node_indices() {
            let node_weight = topology.graph.node_weight(node).unwrap();
            new_index.insert_node(
                topology.node_index_to_id[node.index()].unwrap(),
                node_weight.position.into(),
            );
        }
        for edge_idx in topology.graph.edge_indices() {
            let nodes = topology.graph.edge_endpoints(edge_idx).unwrap();
            let edge_pos_a = topology.graph.node_weight(nodes.0).unwrap().position;
            let edge_pos_b = topology.graph.node_weight(nodes.1).unwrap().position;

            new_index.insert_edge(
                topology.edge_index_to_id[edge_idx.index()].unwrap(),
                edge_pos_a.into(),
                edge_pos_b.into(),
            );
        }

        new_index
    }

    pub fn insert_node(&mut self, id: NodeId, pos: [f32; 2]) {
        self.nodes.insert(SpatialNode { id, point: pos });
    }

    pub fn remove_node(&mut self, node: SpatialNode) {
        self.nodes.remove(&node);
    }

    pub fn nearest_node(&self, x: f32, y: f32) -> Option<NodeId> {
        self.nodes.nearest_neighbor(&[x, y]).map(|n| n.id)
    }

    pub fn nodes_in_radius(&self, x: f32, y: f32, r: f32) -> Vec<NodeId> {
        let r2 = r * r;
        self.nodes
            .locate_within_distance([x, y], r2)
            .map(|n| n.id)
            .collect()
    }

    pub fn insert_edge(&mut self, id: EdgeId, pa: [f32; 2], pb: [f32; 2]) {
        self.edges.insert(SpatialEdge { id, a: pa, b: pb });
    }

    pub fn nearest_edge(&self, x: f32, y: f32) -> Option<EdgeId> {
        self.edges.nearest_neighbor(&[x, y]).map(|e| e.id)
    }
}
