use crate::graphs::{
    edges::{Edge, EdgeData, EdgeProperties},
    nodes::NodeData,
};
use bevy::prelude::Resource;
use petgraph::graph::{NodeIndex, UnGraph};
use serde_json::Value;
use std::collections::{BTreeSet, HashMap};

/// ## Spatio-Temporal Graph (STG)
/// A spatio-temporal graph structure that holds nodes and edges with temporal properties.
/// The node and edge data are minimal structs optimized for graph operations.
/// The nodes map allows quick lookup of node indices by their string IDs.
/// The timestamps vector holds all unique timestamps present in the graph.
#[derive(Debug, Resource, Default)]
pub(crate) struct SpatioTemporalGraph {
    pub(crate) graph: UnGraph<NodeData, EdgeData>,
    pub(crate) nodes_map: HashMap<String, NodeIndex>,
    pub(crate) edges: Vec<Edge>,
    pub(crate) timestamps: Vec<i64>,
}

/// ## Snapshot Graph
/// A snapshot of the spatio-temporal graph at a specific timestamp.
#[derive(Debug, Resource, Default, Clone)]
pub struct SnapshotGraph {
    pub(crate) graph: UnGraph<NodeData, EdgeData>,
    #[allow(dead_code)]
    pub(crate) timestamp: i64,
}

impl SpatioTemporalGraph {
    pub fn new() -> Self {
        SpatioTemporalGraph {
            graph: UnGraph::new_undirected(),
            nodes_map: HashMap::new(),
            edges: Vec::new(),
            timestamps: Vec::new(),
        }
    }

    #[allow(dead_code)]
    pub fn load_from_geojson(geojson_str: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let value: Value = serde_json::from_str(geojson_str)?;
        let mut stg = SpatioTemporalGraph::new();

        if let Some(features) = value.get("features").and_then(|f| f.as_array()) {
            let mut timestamp_set = BTreeSet::new();
            let mut node_features = Vec::new();
            let mut edge_features = Vec::new();

            for feature in features {
                if let Some(geom_type) = feature
                    .get("geometry")
                    .and_then(|g| g.get("type"))
                    .and_then(|t| t.as_str())
                {
                    if geom_type == "Point" {
                        node_features.push(feature.clone());
                    } else if geom_type == "LineString" {
                        edge_features.push(feature.clone());
                        if let Some(props) = feature.get("properties").and_then(|p| p.as_object()) {
                            for key in props.keys() {
                                if let Ok(ts) = key.parse::<i64>() {
                                    timestamp_set.insert(ts);
                                }
                            }
                        }
                    }
                }
            }

            stg.timestamps = timestamp_set.into_iter().collect();

            stg.load_nodes(node_features)?;
            stg.load_edges(edge_features)?;
        }

        Ok(stg)
    }

    #[allow(dead_code)]
    fn load_nodes(&mut self, node_features: Vec<Value>) -> Result<(), Box<dyn std::error::Error>> {
        for feature in node_features {
            if let (Some(id_val), Some(coords)) = (
                feature
                    .get("properties")
                    .and_then(|p| p.get("id"))
                    .and_then(|i| i.as_str()),
                feature
                    .get("geometry")
                    .and_then(|g| g.get("coordinates"))
                    .and_then(|c| c.as_array()),
            ) {
                if coords.len() >= 2 {
                    let lon = coords[0].as_f64().unwrap_or(0.0);
                    let lat = coords[1].as_f64().unwrap_or(0.0);

                    let node_data = NodeData {
                        pos: (lon as f32, lat as f32, 0.0).into(),
                        id: self.graph.node_count(),
                    };

                    let node_idx = self.graph.add_node(node_data);
                    self.nodes_map.insert(id_val.to_string(), node_idx);
                }
            }
        }

        Ok(())
    }

    #[allow(dead_code)]
    fn load_edges(&mut self, edge_features: Vec<Value>) -> Result<(), Box<dyn std::error::Error>> {
        for feature in edge_features {
            if let (Some(props), _coords) = (
                feature.get("properties").and_then(|p| p.as_object()),
                feature
                    .get("geometry")
                    .and_then(|g| g.get("coordinates"))
                    .and_then(|c| c.as_array()),
            ) {
                let source = props
                    .get("source")
                    .and_then(|s| s.as_str())
                    .map(|s| s.to_string());
                let target = props
                    .get("target")
                    .and_then(|t| t.as_str())
                    .map(|t| t.to_string());

                if let (Some(src), Some(tgt)) = (source, target) {
                    let mut time_props: HashMap<i64, EdgeProperties> = HashMap::new();

                    for (key, value) in props {
                        if let Ok(ts) = key.parse::<i64>() {
                            let diameter = value.get("diameter").and_then(|d| d.as_f64());
                            let other = if let Value::Object(map) = value {
                                map.clone()
                            } else {
                                serde_json::Map::new()
                            };

                            time_props.insert(
                                ts,
                                EdgeProperties {
                                    diameter,
                                    other: other.into_iter().map(|(k, v)| (k, v)).collect(),
                                },
                            );
                        }
                    }

                    let edge = Edge {
                        source: src,
                        target: tgt,
                        properties: time_props,
                    };

                    self.edges.push(edge);
                }
            }
        }

        Ok(())
    }

    pub fn snapshot_at(&self, timestamp: i64) -> Result<SnapshotGraph, Box<dyn std::error::Error>> {
        // Create a new undirected graph for the snapshot
        let mut snapshot_graph = UnGraph::new_undirected();

        // Add all nodes to the snapshot graph
        for node in self.graph.node_weights() {
            snapshot_graph.add_node(*node);
        }

        for edge in &self.edges {
            // Check if both source and target nodes exist in the graph
            if let (Some(&src_idx), Some(&tgt_idx)) = (
                self.nodes_map.get(&edge.source),
                self.nodes_map.get(&edge.target),
            ) {
                // Check if the edge has properties for the given timestamp
                if edge.properties.contains_key(&timestamp) {
                    // Add the edge to the snapshot graph
                    if let (Some(src_node), Some(tgt_node)) = (
                        self.graph.node_weight(src_idx),
                        self.graph.node_weight(tgt_idx),
                    ) {
                        snapshot_graph.add_edge(
                            NodeIndex::new(src_idx.index()),
                            NodeIndex::new(tgt_idx.index()),
                            EdgeData {
                                node_poss: (src_node.pos, tgt_node.pos),
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
            stg.nodes_map.insert(idx.to_string(), node_idx);
        }

        stg.graph.add_edge(
            NodeIndex::new(0),
            NodeIndex::new(1),
            EdgeData {
                node_poss: (nodes[0].pos, nodes[1].pos),
            },
        );

        stg.timestamps = vec![1000000000000000000i64];

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
            EdgeData {
                node_poss: (nodes[0].pos, nodes[1].pos),
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
