use std::collections::HashMap;

use glam::Vec2;
use petgraph::prelude::*;

pub type NodeId = u64;
pub type EdgeId = u64;


#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct VisualNode {
    pub position: Vec2,
    pub color: [f32; 3],
    radius: f32,
}


#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct VisualEdge {
    pub color: [f32; 3],
    pub width: f32,
}

#[derive(Debug)]
pub struct GraphTopology {
    pub graph: Graph<VisualNode, VisualEdge, Undirected>,

    node_id_to_index: std::collections::HashMap<NodeId, NodeIndex>,
    edge_id_to_index: std::collections::HashMap<EdgeId, EdgeIndex>,

    node_index_to_id: Vec<Option<NodeId>>,
    edge_index_to_id: Vec<Option<EdgeId>>,

    node_next_id: NodeId,
    edge_next_id: EdgeId,
}

impl GraphTopology {
    pub fn empty() -> Self {
        Self {
            graph: Graph::default(),

            node_id_to_index: HashMap::new(),
            node_index_to_id: Vec::new(),
            node_next_id: 0,

            edge_id_to_index: HashMap::new(),
            edge_index_to_id: Vec::new(),
            edge_next_id: 0,
        }
    }

    pub fn add_node(&mut self, node: VisualNode) -> NodeId {
        let id = self.node_next_id;
        self.node_next_id += 1;

        let idx = self.graph.add_node(node);

        if idx.index() >= self.node_index_to_id.len() {
            self.node_index_to_id.resize(idx.index() + 1, None);
        }

        self.node_index_to_id[idx.index()] = Some(id);
        self.node_id_to_index.insert(id, idx);

        id
    }

    pub fn remove_node(&mut self, id: NodeId) {
        if let Some(idx) = self.node_id_to_index.remove(&id) {
            self.graph.remove_node(idx);
            self.node_index_to_id[idx.index()] = None;
        }
    }

    pub fn remove_edge(&mut self, id: EdgeId) {
        if let Some(idx) = self.edge_id_to_index.remove(&id) {
            self.graph.remove_edge(idx);
            self.edge_index_to_id[idx.index()] = None;
        }
    }

    pub fn add_edge(&mut self, a: NodeId, b: NodeId, edge: VisualEdge) -> EdgeId {
        let idx_a = self.node_id_to_index[&a];
        let idx_b = self.node_id_to_index[&b];
        let id = self.edge_next_id;

        let idx = self.graph.add_edge(idx_a, idx_b, edge);

        if idx.index() >= self.edge_index_to_id.len() {
            self.edge_index_to_id.resize(idx.index() + 1, None);
        }

        self.edge_index_to_id[idx.index()] = Some(id);
        self.edge_id_to_index.insert(id, idx);
        id
    }

    pub fn demo() -> Self {
        let mut triangle_graph = GraphTopology::empty();
        let node_color = [0.2, 0.7, 1.0];
        let edge_color = [1.0, 1.0, 1.0];

        let n0 = triangle_graph.add_node(VisualNode {
            position: Vec2::new(-0.5, -0.5),
            color: node_color,
            radius: 0.05,
        });
        let n1 = triangle_graph.add_node(VisualNode {
            position: Vec2::new(0.5, -0.5),
            color: node_color.clone(),
            radius: 0.05,
        });
        let n2 = triangle_graph.add_node(VisualNode {
            position: Vec2::new(0.0, 0.5),
            color: node_color.clone(),
            radius: 0.05,
        });
        triangle_graph.add_edge(
            n0,
            n1,
            VisualEdge {
                color: edge_color.clone(),
                width: 0.02,
            },
        );
        triangle_graph.add_edge(
            n1,
            n2,
            VisualEdge {
                color: edge_color.clone(),
                width: 0.02,
            },
        );
        triangle_graph.add_edge(
            n2,
            n0,
            VisualEdge {
                color: [0.5, 0.5, 0.5],
                width: 0.02,
            },
        );
        triangle_graph
    }
}
