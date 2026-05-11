// use crate::app::*;
use crate::app::app_model::AppModel;
use crate::app::context::AppContext;
use crate::app::controller;
use crate::app::app_state::AppState;
use crate::app::messages::AppEvent;
use crate::app::app_state::Selection;

#[test]
fn set_time_updates_state() {
    let mut app = AppState::default();
    let mut ctx = AppContext::new();

    controller::handle_event(
        &mut app,
        AppEvent::SetTimeIndex(3),
        &mut ctx,
    );

    assert_eq!(app.graph_time, Some(3));
    assert_eq!(ctx.commands.len(), 1);
}

#[test]
fn selecting_node_clears_edge() {
    let mut app = AppState::default();
    let mut ctx = AppContext::new();

    controller::handle_event(
        &mut app,
        AppEvent::SelectEdge(10),
        &mut ctx,
    );

    controller::handle_event(
        &mut app,
        AppEvent::SelectNode(5),
        &mut ctx,
    );

    assert_eq!(app.selection, Some(Selection::Node(5)));
    // assert_eq!(app.selection, None);
}

#[test]
fn clear_selection() {
    let mut app = AppState::default();
    let mut ctx = AppContext::new();

    controller::handle_event(
        &mut app,
        AppEvent::SelectNode(1),
        &mut ctx,
    );

    controller::handle_event(
        &mut app,
        AppEvent::ClearSelection,
        &mut ctx,
    );

    assert_eq!(app.selection, None);
}