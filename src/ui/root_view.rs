use gpui::{App, Context, FocusHandle, IntoElement, ParentElement, Render, Styled, div, rgb};
use crate::app::app_model::AppModel;

pub struct RootView {
    focus_handle: FocusHandle,
}

impl RootView {
    pub fn new(cx: &mut Context<'_, Self>) -> Self {
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
        _window: &mut gpui::Window,
        _cx: &mut Context<'_, Self>,
    ) -> impl IntoElement {
        let app  = _cx.global::<AppModel>();

        div().flex().size_full().bg(rgb(0x1a1a1a)).child(
            div()
                .child("Graph Viewer Placeholder")
                .text_color(rgb(0xffffff)),
        )
    }
}
