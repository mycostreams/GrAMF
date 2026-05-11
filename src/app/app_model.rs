use crate::app::context::AppContext;

pub struct AppModel {
    ctx: AppContext,
    // pub graph: GraphState, --- IGNORE ---
    // pub camera: Camera, --- IGNORE ---
}

impl AppModel {
    pub fn new() -> Self {
        Self {
            ctx: AppContext::new(),
            // graph: GraphState::new(), --- IGNORE ---
            // camera: Camera::new(), --- IGNORE ---
        }
    }
}
