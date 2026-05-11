// app/controller.rs

use crate::app::{
    app_state::{AppState, Selection},
    context::AppContext,
    messages::{AppCommand, AppEvent},
};

pub fn handle_event(app: &mut AppState, event: AppEvent, ctx: &mut AppContext) {
    match event {
        AppEvent::SetTimeIndex(t) => {
            app.graph_time = Some(t);
            ctx.enqueue(AppCommand::Redraw);
        }

        AppEvent::SelectNode(id) => {
            app.selection = Some(Selection::Node(id));
            ctx.enqueue(AppCommand::Redraw);
        }

        AppEvent::SelectEdge(id) => {
            app.selection = Some(Selection::Edge(id));
            ctx.enqueue(AppCommand::Redraw);
        }

        AppEvent::ClearSelection => {
            app.selection = None;
            ctx.enqueue(AppCommand::Redraw);
        }
    }
}
