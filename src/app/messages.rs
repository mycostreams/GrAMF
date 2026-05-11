// app/messages.rs

#[derive(Debug, Clone)]
pub enum AppEvent {
    SetTimeIndex(usize),

    SelectNode(usize),
    SelectEdge(usize),

    ClearSelection,
}

#[derive(Debug, Clone)]
pub enum AppCommand {
    Redraw,
}
