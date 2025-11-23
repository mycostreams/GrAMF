use std::collections::HashMap;
use bevy::math::Vec3;
use chrono::{NaiveDate, NaiveDateTime};
use petgraph::graph::{NodeIndex, UnGraph};

#[derive(Debug, Clone)]
pub(crate) struct EdgeData {
    width: f32,
}
#[derive(Debug, Clone, Default, Copy)]

pub(crate) struct NodeData {
    pub(crate) pos: Vec3,
    pub(crate) id: usize,
}

#[derive(Debug)]
pub(crate) struct SpatioTemporalGraph {
    pub(crate) graph: UnGraph<NodeData, EdgeData>,
    timestamps: HashMap<i32, NaiveDateTime>,
}

impl From<NodeData> for NodeIndex {
    fn from(value: NodeData) -> Self {
        NodeIndex::new(value.id)
    }
}

pub(crate) fn return_simple_graph() -> SpatioTemporalGraph {
    let node1 = NodeData {
        pos: Vec3 {
            x: 0.0,
            y: 1.0,
            z: 0.0,
        },
        id: 0,
    };
    let node2 = NodeData {
        pos: Vec3::new(1.0, 0.0, 0.0),
        id: 1,
    };
    let node3: NodeData = NodeData {
        pos: Vec3 {
            x: 1.0,
            y: 1.0,
            z: 0.0,
        },
        id: 2,
    };

    let mut g = UnGraph::<NodeData, EdgeData>::new_undirected();
    for node in [node1, node2, node3] {
        g.add_node(node);
    }
    for edge in [(0, 1), (1,2), (2,0)] {
        g.add_edge(edge.0.into(), edge.1.into(), EdgeData { width: 5.0 });
    }
    let mut timestamps = HashMap::<i32, NaiveDateTime>::new();
    let time1 = NaiveDate::from_ymd_opt(2016, 7, 8)
        .unwrap()
        .and_hms_opt(9, 10, 11)
        .unwrap();

    timestamps.insert(0, time1);

    return SpatioTemporalGraph {
        graph: g,
        timestamps: timestamps,
    };
}
