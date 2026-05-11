use crate::app::AppEvent;
use crate::ui::helpers::dispatch;
use crate::{app::gpui_app::AppModel, ui::root_view::RootView};
use gpui::*;
use gpui_component::button::{Button, DropdownButton};

actions!(file_menu_actions, [Open, Save, Exit,]);

pub fn menu_bar(cx: &mut Context<'_, RootView>) -> impl IntoElement {
    div().flex_row().bg(rgb(0x333333)).child(
        DropdownButton::new("file")
            .button(Button::new("File").label("File..."))
            .dropdown_menu(|menu, _, _| {
                menu.menu("Open", Box::new(Open))
                    .separator()
                    .menu("Quit", Box::new(Exit))
            }),
    )
}
