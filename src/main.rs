use gpui::{App, AppContext, Application, Bounds, WindowBounds, WindowOptions, px, size};

// use crate::app::AppModel;

mod app;
mod graph;
mod render;
mod ui;
mod test;

fn main() {
    Application::new().run(|app: &mut App| {
        let bounds = Bounds::centered(None, size(px(800.), px(600.)), app);

        app.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |window, app| {
                let focus_handle = app.focus_handle();
                focus_handle.focus(window);

                app.new(ui::root_view::RootView::new)
            },
        )
        .unwrap();
        app.activate(true);
        app.on_window_closed(|app| {
            app.quit();
        })
        .detach();
    });
}
