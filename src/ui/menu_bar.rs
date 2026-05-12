

// pub fn menu_bar(_cx: &mut App) -> impl IntoElement {
//     div()
//         .flex_row()
//         .bg(rgb(0x333333))
//         .child(
//             DropdownButton::new("file")
//                 .button(Button::new("File").label("File..."))
//                 .dropdown_menu(|menu, _, _| {
//                     menu.menu("Open", Box::new(Open))
//                         .separator()
//                         .menu("Quit", Box::new(Exit))
//                 }),
//         )
//         .on_action(|&Exit, _window, _app| {
//             _app.quit();
//         })
// }
