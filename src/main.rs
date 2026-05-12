use gpui::*;
use gpui_component::{button::*, *};
use gpui_platform;
// use crate::app::AppModel;
use gpui_component_assets;

use crate::app::gpui_app::AppModel;

mod app;
mod test;
mod ui;

fn main() {
    let app = gpui_platform::application().with_assets(gpui_component_assets::Assets);

    app.run(move |app| {
        gpui_component::init(app);
        ui::init(app);

        let bounds = Bounds::centered(None, size(px(800.), px(600.)), app);
        app.spawn(async move |cx| {
            cx.open_window(
                WindowOptions {
                    titlebar: Some(TitlebarOptions {
                        title: Some("GrAMF".into()),
                        appears_transparent: true,
                        traffic_light_position: None,
                    }),
                    window_bounds: Some(gpui::WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                |window, cx| {
                    let view = cx.new(|cx| ui::root_view::RootView::new(cx));
                    cx.new(|cx| Root::new(view, window, cx))
                },
            )
            .unwrap();
            // cx.activate(true);
        })
        .detach();
    });
}
