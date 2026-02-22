use glam::Vec2;
use petgraph::prelude::*;
use redb::Database;
pub struct NodeData {
    pub id: u32,
    pub position: Vec2,
    pub value: f32,
    pub radius: f32,
}

pub struct EdgeData {
    // pub src: u32,
    // pub trg: u32,
    pub value: f32,
    pub width: f32,
}

pub struct GraphModel {
    pub graph: Graph<NodeData, EdgeData>,
    pub db: Option<Database>,
}

impl GraphModel {
    pub fn demo() -> Self {
        let mut triangle_graph = Graph::new();
        let n0 = triangle_graph.add_node(NodeData {
            id: 0,
            position: Vec2::new(-0.5, -0.5),
            value: 0.0,
            radius: 0.05,
        });
        let n1 = triangle_graph.add_node(NodeData {
            id: 1,
            position: Vec2::new(0.5, -0.5),
            value: 0.0,
            radius: 0.05,
        });
        let n2 = triangle_graph.add_node(NodeData {
            id: 2,
            position: Vec2::new(0.0, 0.5),
            value: 0.0,
            radius: 0.05,
        });
        triangle_graph.add_edge(
            n0,
            n1,
            EdgeData {
                // src: 0,
                // trg: 1,
                value: 1.0,
                width: 0.02,
            },
        );
        triangle_graph.add_edge(
            n1,
            n2,
            EdgeData {
                // src: 1,
                // trg: 2,
                value: 0.75,
                width: 0.02,
            },
        );
        triangle_graph.add_edge(
            n2,
            n0,
            EdgeData {
                // src: 2,
                // trg: 0,
                value: 0.5,
                width: 0.02,
            },
        );
        Self {
            graph: triangle_graph,
            db: None,
        }
    }
}
