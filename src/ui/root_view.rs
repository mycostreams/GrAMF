use gpui::{App, Context, Entity, FocusHandle, IntoElement, ParentElement, Render, Styled, div, rgb};
use gpui_component::menu::AppMenuBar;

use crate::{app::gpui_app::AppModel, ui::title_bar::{self, init_app_menu}};


pub struct RootView {
    focus_handle: FocusHandle,
    app_menu_bar: Entity<AppMenuBar>,
}

impl RootView {
    pub fn new(cx: &mut App) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            app_menu_bar: init_app_menu("GrAMF", cx),
        }
    }
}

impl gpui::Focusable for RootView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for RootView {
    fn render(
        &mut self,
        _window: &mut gpui::Window,
        cx: &mut Context<'_, Self>,
    ) -> impl IntoElement {
        let _app = cx.global::<AppModel>();

        div()
        .bg(rgb(0x1a1a1a))
            .child(
                title_bar::title_bar(cx, self.app_menu_bar.clone())
            )
            .flex()
            .size_full()
            .bg(rgb(0x1a1a1a))
            .child(
                div()
                    .child("Graph Viewer Placeholder")
                    .text_color(rgb(0xffffff)),
            )
    }
}
