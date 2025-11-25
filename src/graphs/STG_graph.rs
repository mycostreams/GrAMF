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

#[test]
fn test_default_graph() {
    let test_graph = SpatioTemporalGraph::default();
    assert_eq!(test_graph.graph.edge_count(), 0);
    assert_eq!(test_graph.graph.node_count(), 0);
}

// pub(crate) fn return_simple_graph() -> SpatioTemporalGraph {
//     let node1 = NodeData {
//         pos: Vec3 {
//             x: 0.0,
//             y: 10.0,
//             z: 0.0,
//         },
//         id: 0,
//     };
//     let node2 = NodeData {
//         pos: Vec3::new(10.0, 0.0, 0.0),
//         id: 1,
//     };
//     let node3: NodeData = NodeData {
//         pos: Vec3 {
//             x: 10.0,
//             y: 10.0,
//             z: 0.0,
//         },
//         id: 2,
//     };

//     let mut g = UnGraph::<NodeData, EdgeData>::new_undirected();
//     for node in [node1, node2, node3] {
//         g.add_node(node);
//     }
//     for edge in [(0, 1), (1, 2), (2, 0)] {
//         g.add_edge(edge.0.into(), edge.1.into(), EdgeData { width: 5.0 });
//     }
//     let mut timestamps = HashMap::<i32, NaiveDateTime>::new();
//     let time1 = NaiveDate::from_ymd_opt(2016, 7, 8)
//         .unwrap()
//         .and_hms_opt(9, 10, 11)
//         .unwrap();

//     timestamps.insert(0, time1);

//     return SpatioTemporalGraph {
//         graph: g,
//         timestamps: timestamps,
//     };
// }
