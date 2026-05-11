use crate::app::{AppEvent, gpui_app::AppModel};
use gpui::*;

pub fn dispatch(cx: &mut gpui::Context<'_, AppModel>, event: AppEvent) {
    cx.update_global::<AppModel, _>(|app, cx| {
        app.dispatch(event, cx);
    });
}
