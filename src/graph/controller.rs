use std::collections::HashMap;

use polars::prelude::NamedFrom;
use polars::series::Series;

use crate::graph::properties::PropertyStore;
use crate::graph::r_tree::index_controller::SpatialIndex;
use crate::graph::topology::NodeId;
use crate::graph::topology::{EdgeId, GraphTopology, VisualEdge, VisualNode};

pub struct GraphEngine {
    pub topology: GraphTopology,
    pub rtree: SpatialIndex,
    pub properties: PropertyStore,
}

impl GraphEngine {
    pub fn new() -> Self {
        Self {
            topology: GraphTopology::empty(),
            properties: PropertyStore::new(),
            rtree: SpatialIndex::new(),
        }
    }

    pub fn demo() -> Self {
        Self {
            topology: GraphTopology::demo(),
            properties: PropertyStore::new(),
            rtree: SpatialIndex::from_topology(GraphTopology::demo()),
        }
    }

    pub fn add_node(&mut self, visual: VisualNode, properties: Vec<Series>) -> NodeId {
        let id = self.topology.add_node(visual);
        self.properties.insert_node(id, properties).unwrap();
        self.rtree.insert_node(id, visual.position.into());
        id
    }

    pub fn add_json_node(
        &mut self,
        visual: VisualNode,
        properties: HashMap<&str, serde_json::Value>,
    ) -> NodeId {
        let prop_series = properties
            .into_iter()
            .map(|(name, value)| match value {
                serde_json::Value::String(s) => Series::new(name.into(), &[s]),
                serde_json::Value::Number(n) => {
                    Series::new(name.into(), &[n.as_f64().unwrap_or(0.0)])
                }
                serde_json::Value::Bool(b) => Series::new(name.into(), &[b]),
                _ => Series::new(name.into(), &[value.to_string()]),
            })
            .collect();
        self.add_node(visual, prop_series)
    }

    pub fn remove_node(&mut self, id: NodeId) {
        self.topology.remove_node(id);
        self.properties.mark_inactive(id).unwrap();
        // Note: rtree removal not implemented yet
    }

    pub fn add_edge(&mut self, source: NodeId, target: NodeId, visual: VisualEdge) -> EdgeId {
        let edge_id = self.topology.add_edge(source, target, visual);
        // Update rtree with edge - need node positions
        if let (Some(source_node), Some(target_node)) = (
            self.topology
                .graph
                .node_weight(self.topology.node_id_to_index[&source]),
            self.topology
                .graph
                .node_weight(self.topology.node_id_to_index[&target]),
        ) {
            self.rtree.insert_edge(
                edge_id,
                [source_node.position.x, source_node.position.y],
                [target_node.position.x, target_node.position.y],
            );
        }
        edge_id
    }

    pub fn compute_subgraph(
        &self,
        seed: NodeId,
    ) -> polars::prelude::PolarsResult<polars::prelude::DataFrame> {
        let ids = self.topology.bfs(seed);
        self.properties.filter_nodes(&ids)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec2;
    use std::collections::HashMap;

    #[test]
    fn test_add_node_and_properties() {
        let mut engine = GraphEngine::new();
        let visual = VisualNode {
            position: Vec2::ZERO,
            color: [1.0, 0.0, 0.0],
            radius: 1.0,
        };
        let props = vec![
            Series::new("foo".into(), &[123i32]),
            Series::new("bar".into(), &[true]),
        ];
        let id = engine.add_node(visual, props);
        let df = &engine.properties.nodes;
        assert_eq!(df.height(), 1);
        assert_eq!(
            df.column("node_id")
                .unwrap()
                .get(0)
                .unwrap()
                .extract::<u64>()
                .unwrap(),
            id
        );
        assert_eq!(
            df.column("foo")
                .unwrap()
                .get(0)
                .unwrap()
                .extract::<i32>()
                .unwrap(),
            123
        );
        assert_eq!(
            df.column("bar")
                .unwrap()
                .get(0)
                .unwrap()
                .extract_bool()
                .unwrap(),
            true
        );
    }

    #[test]
    fn test_add_json_node() {
        let mut engine = GraphEngine::new();
        let visual = VisualNode {
            position: Vec2::ONE,
            color: [0.0, 1.0, 0.0],
            radius: 2.0,
        };
        let mut props = HashMap::new();
        props.insert("name", serde_json::Value::String("abc".to_string()));
        props.insert(
            "score",
            serde_json::Value::Number(serde_json::Number::from(7)),
        );
        props.insert("flag", serde_json::Value::Bool(true));
        let id = engine.add_json_node(visual, props);
        let df = &engine.properties.nodes;
        assert_eq!(df.height(), 1);
        assert_eq!(
            df.column("node_id")
                .unwrap()
                .get(0)
                .unwrap()
                .extract::<u64>()
                .unwrap(),
            id
        );
        assert_eq!(
            df.column("name").unwrap().get(0).unwrap().str_value(),
            "abc"
        );
        assert_eq!(
            df.column("score")
                .unwrap()
                .get(0)
                .unwrap()
                .extract::<f64>()
                .unwrap(),
            7.0
        );
        assert_eq!(
            df.column("flag")
                .unwrap()
                .get(0)
                .unwrap()
                .extract_bool()
                .unwrap(),
            true
        );
    }

    #[test]
    fn test_remove_node_marks_inactive() {
        let mut engine = GraphEngine::new();
        let visual = VisualNode {
            position: Vec2::ZERO,
            color: [1.0, 0.0, 0.0],
            radius: 1.0,
        };
        let id = engine.add_node(visual, vec![]);
        engine.remove_node(id);
        let df = &engine.properties.nodes;
        assert_eq!(df.height(), 1);
        let active_val = df
            .column("is_active")
            .unwrap()
            .get(0)
            .unwrap()
            .extract_bool()
            .unwrap();
        assert_eq!(active_val, false);
    }

    #[test]
    fn test_compute_subgraph() {
        let mut engine = GraphEngine::new();
        let v0 = VisualNode {
            position: Vec2::ZERO,
            color: [1.0, 0.0, 0.0],
            radius: 1.0,
        };
        let v1 = VisualNode {
            position: Vec2::ONE,
            color: [0.0, 1.0, 0.0],
            radius: 1.0,
        };
        let id0 = engine.add_node(v0, vec![]);
        let id1 = engine.add_node(v1, vec![]);
        engine.topology.add_edge(
            id0,
            id1,
            crate::graph::topology::VisualEdge {
                color: [1.0, 1.0, 1.0],
                width: 1.0,
            },
        );
        let df = engine.compute_subgraph(id0).unwrap();
        let node_ids: Vec<_> = df
            .column("node_id")
            .unwrap()
            .u64()
            .unwrap()
            .into_no_null_iter()
            .collect();
        assert!(node_ids.contains(&id0));
        assert!(node_ids.contains(&id1));
    }
}
