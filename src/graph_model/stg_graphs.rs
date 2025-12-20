use crate::graph_model::{edges::EdgeLight, nodes::NodeData};
use bevy::{math::Vec3, prelude::Resource};
use petgraph::graph::{NodeIndex, UnGraph};

#[derive(Debug, Default)]
pub struct Metadata {
    timestamps: Vec<i64>,
}

#[derive(Debug, Default)]
#[allow(dead_code)]
pub struct Spore {
    id: String,
    position: Vec3,
}

/// ## Spatio-Temporal Graph (STG)
/// A spatio-temporal graph structure that holds nodes and edges with temporal properties.
/// The node and edge data are minimal structs optimized for graph operations.
/// The nodes map allows quick lookup of node indices by their string IDs.
/// The timestamps vector holds all unique timestamps present in the graph.
#[derive(Debug, Default)]
pub struct SpatioTemporalGraph {
    pub graph: UnGraph<NodeData, EdgeLight>,
    pub nodes_map: std::collections::HashMap<usize, NodeIndex>,
    pub spores: Vec<Spore>,
    pub metadata: Metadata,
}

/// ## Snapshot Graph
/// A snapshot of the spatio-temporal graph at a specific timestamp.
#[derive(Debug, Resource, Default, Clone)]
pub struct SnapshotGraph {
    pub graph: UnGraph<NodeData, EdgeLight>,
    pub timestamp: i64,
}

impl SpatioTemporalGraph {
    pub fn new() -> Self {
        SpatioTemporalGraph {
            graph: UnGraph::new_undirected(),
            spores: Vec::new(),
            metadata: Metadata::default(),
            nodes_map: std::collections::HashMap::new(),
        }
    }

    pub fn snapshot_at(&self, timestamp: i64) -> Result<SnapshotGraph, Box<dyn std::error::Error>> {
        // Create a new undirected graph for the snapshot
        let mut snapshot_graph = UnGraph::new_undirected();

        // Add all nodes to the snapshot graph
        for node in self.graph.node_weights() {
            snapshot_graph.add_node(*node);
        }

        for edge in self.graph.edge_weights().collect::<Vec<&EdgeLight>>() {
            // Check if both source and target nodes exist in the graph
            if let (Some(&src_idx), Some(&tgt_idx)) = (
                self.nodes_map.get(&edge.source),
                self.nodes_map.get(&edge.target),
            ) {
                // Check if the graph has properties for the given timestamp
                if self.metadata.timestamps.contains(&timestamp) {
                    // Add the edge to the snapshot graph
                    if let (Some(src_node), Some(tgt_node)) = (
                        self.graph.node_weight(src_idx),
                        self.graph.node_weight(tgt_idx),
                    ) {
                        snapshot_graph.add_edge(
                            NodeIndex::new(src_idx.index()),
                            NodeIndex::new(tgt_idx.index()),
                            EdgeLight {
                                node_poss: (src_node.pos, tgt_node.pos),
                                source: edge.source,
                                target: edge.target,
                                id: edge.id,
                            },
                        );
                    }
                }
            }
        }

        Ok(SnapshotGraph {
            graph: snapshot_graph,
            timestamp,
        })
    }

    pub fn connected_subgraph(
        &self,
        start_node: NodeIndex,
        timestamp: i64,
    ) -> Result<Vec<NodeIndex>, Box<dyn std::error::Error>> {
        let snapshot = self.snapshot_at(timestamp)?;
        let mut visited = std::collections::HashSet::new();
        let mut queue = std::collections::VecDeque::new();
        let mut component = Vec::new();

        if snapshot.graph.node_count() == 0 {
            return Ok(component);
        }

        queue.push_back(start_node);
        visited.insert(start_node);

        while let Some(current) = queue.pop_front() {
            component.push(current);

            for neighbor in snapshot.graph.neighbors(current) {
                if !visited.contains(&neighbor) {
                    visited.insert(neighbor);
                    queue.push_back(neighbor);
                }
            }
        }

        Ok(component)
    }

    pub fn generate_simple() -> Self {
        let mut stg = SpatioTemporalGraph::new();

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

        for (idx, node) in nodes.iter().enumerate() {
            let node_idx = stg.graph.add_node(*node);
            stg.nodes_map.insert(idx, node_idx);
        }

        stg.graph.add_edge(
            NodeIndex::new(0),
            NodeIndex::new(1),
            EdgeLight {
                node_poss: (nodes[0].pos, nodes[1].pos),
                source: 0,
                target: 1,
                id: 0,
            },
        );

        stg
    }
}

impl SnapshotGraph {
    pub fn new() -> Self {
        SnapshotGraph {
            graph: UnGraph::new_undirected(),
            timestamp: 0,
        }
    }

    pub fn generate_simple() -> Self {
        let mut stg = SnapshotGraph::new();

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

        for node in nodes.iter() {
            stg.graph.add_node(*node);
        }

        stg.graph.add_edge(
            NodeIndex::new(0),
            NodeIndex::new(1),
            EdgeLight {
                node_poss: (nodes[0].pos, nodes[1].pos),
                source: 0,
                target: 1,
                id: 0,
            },
        );

        stg.timestamp = 1000000000000000000i64;

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

#[test]
fn test_snapshot_at() {
    let stg = SpatioTemporalGraph::generate_simple();
    let snapshot = stg.snapshot_at(1000000000000000000i64);
    assert!(snapshot.is_ok());
}

#[test]
fn test_connected_subgraph() {
    let stg = SpatioTemporalGraph::generate_simple();
    let component = stg.connected_subgraph(NodeIndex::new(0), 1000000000000000000i64);
    assert!(component.is_ok());
}
