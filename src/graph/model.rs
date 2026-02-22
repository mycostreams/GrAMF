use glam::Vec2;

pub struct GraphModel {
    pub nodes: Vec<Vec2>,
    pub edges: Vec<(u32, u32)>,
}

impl GraphModel {
    pub fn demo() -> Self {
        Self {
            nodes: vec![
                Vec2::new(-0.5, -0.5),
                Vec2::new(0.5, -0.5),
                Vec2::new(0.0, 0.5),
            ],
            edges: vec![(0, 1), (1, 2), (2, 0)],
        }
    }
}
