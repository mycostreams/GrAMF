// pub enum GraphTime {
//     RealTime(f32),
//     FrameTime(usize),
// }

#[derive(Debug, Clone, PartialEq)]
pub enum Selection {
    Node(usize),
    Edge(usize),
}

#[derive(Debug, Default)]
pub struct AppState {
    pub graph_time: Option<usize>,
    pub selection: Option<Selection>,
}
    