// app/gpui_app.rs

use gpui::*;

use crate::app;

pub struct AppModel {
    pub state: app::AppState,
}

impl AppModel {
    pub fn new() -> Self {
        Self {
            state: app::AppState::default(),
        }
    }

    pub fn dispatch(&mut self, event: app::AppEvent, cx: &mut Context<'_, Self>) {
        let mut ctx = app::AppContext::new();

        // Phase 1 logic (unchanged)
        app::controller::handle_event(&mut self.state, event, &mut ctx);
        app::lifecycle::process_commands(&self.state, &mut ctx);

        // tell GPUI to re-render
        cx.notify();
    }
}