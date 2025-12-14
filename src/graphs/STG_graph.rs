use crate::graphs::{edges::EdgeData, nodes::NodeData};
use bevy::prelude::Resource;
use chrono::NaiveDateTime;
use petgraph::graph::UnGraph;
use std::collections::HashMap;

#[derive(Debug, Resource, Default)]
pub(crate) struct SpatioTemporalGraph {
    pub(crate) graph: UnGraph<NodeData, EdgeData>,
    timestamps: HashMap<i32, NaiveDateTime>,
}

impl SpatioTemporalGraph {
    pub fn generate_simple() -> Self {
        let mut stg = SpatioTemporalGraph::default();

        let nodes = [
            NodeData {
                pos: (10.0, 0.0, 0.0).into(),
                id: 0,
            },
            NodeData {
                pos: (0.0, 10.0, 0.0).into(),
                id: 1,
            },
            NodeData {
                pos: (10.0, 10.0, 0.0).into(),
                id: 2,
            },
        ];

        for node in nodes {
            stg.graph.add_node(node);
        }

        stg.graph.add_edge(
            petgraph::graph::NodeIndex::new(0),
            petgraph::graph::NodeIndex::new(1),
            EdgeData { width: 1.0, node_poss: (nodes[0].pos, nodes[1].pos) },
        );

        stg
    }
}

#[test]
fn test_default_graph() {
    let test_graph = SpatioTemporalGraph::default();
    assert_eq!(test_graph.graph.edge_count(), 0);
    assert_eq!(test_graph.graph.node_count(), 0);
}

#[test]
fn test_generate_simple() {
    let test_graph = SpatioTemporalGraph::generate_simple();
    assert_eq!(test_graph.graph.node_count(), 3);
    assert_eq!(test_graph.graph.edge_count(), 1);
}