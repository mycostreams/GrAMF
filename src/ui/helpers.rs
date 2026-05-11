use gpui::*;
use crate::app::{AppEvent, gpui_app::AppModel};

pub fn dispatch(cx: &mut ViewContext<impl 'static>, event: AppEvent) {
    cx.update_global::<AppModel, _>(|app, cx| {
        app.dispatch(event, cx);
    });
}