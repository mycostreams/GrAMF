pub mod helpers;
mod menu_bar;
pub mod root_view;
pub mod title_bar;
use crate::{AppModel, ui::title_bar::ExitApp};

pub fn init(cx: &mut gpui::App) {
    cx.set_global::<AppModel>(AppModel::new());
    cx.activate(true);
    cx.on_action(|&ExitApp, _app| _app.quit());
    cx.on_window_closed(|cx, _| {
        cx.quit();
    })
    .detach();

}

