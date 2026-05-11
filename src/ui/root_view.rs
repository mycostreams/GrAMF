use gpui::{App, Context, FocusHandle, IntoElement, ParentElement, Render, Styled, div, rgb};

use crate::app::gpui_app::AppModel;

use super::menu_bar;

pub struct RootView {
    focus_handle: FocusHandle,
}

impl RootView {
    pub fn new(cx: &mut App) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
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
        window: &mut gpui::Window,
        cx: &mut Context<'_, Self>,
    ) -> impl IntoElement {
        let app = cx.global::<AppModel>();

        div()
            .flex()
            .size_full()
            .child(menu_bar::menu_bar(cx))
            .bg(rgb(0x1a1a1a))
            .child(
                div()
                    .child("Graph Viewer Placeholder")
                    .text_color(rgb(0xffffff)),
            )
    }
}
