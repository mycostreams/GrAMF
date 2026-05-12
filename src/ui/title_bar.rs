use gpui::{App, Entity, IntoElement, Menu, MenuItem, ParentElement, SharedString, Styled, actions, div};
use gpui_component::{
    TitleBar, menu::AppMenuBar
};

actions!(file_menu_actions, [NewGraph, OpenGraph, SaveGraph, ExitApp,]);
actions!(edit_menu_actions, [Undo, Redo,]);


fn build_menus(_title: impl Into<SharedString>, _cx: &App) -> Vec<Menu> {  
    vec![  
        Menu {  
            name: "File".into(),  
            items: vec![  
                MenuItem::action("New", NewGraph),  
                MenuItem::action("Open...", OpenGraph),  
                MenuItem::action("Save", SaveGraph), 
                MenuItem::Separator,  
                MenuItem::action("Exit", ExitApp), 
            ],  
            disabled: false,  
        },  
        Menu {  
            name: "Edit".into(),  
            items: vec![  
                MenuItem::action("Undo", Undo),  
                MenuItem::action("Redo", Redo),  
            ],  
            disabled: false,  
        },  
    ]  
} 

// 3. Initialize and set menus globally  
pub fn init_app_menu(title: impl Into<SharedString>, cx: &mut App) -> Entity<AppMenuBar> {  
    let app_menu_bar = AppMenuBar::new(cx);  
    let title: SharedString = title.into();  
      
    cx.set_menus(build_menus(title.clone(), cx));  

    app_menu_bar.update(cx, |menu_bar, cx| {  
        menu_bar.reload(cx);  
    });  
      
    app_menu_bar  
}  


pub fn title_bar(_cx: &mut App, app_menu_bar: Entity<AppMenuBar>) -> impl IntoElement {
    TitleBar::new()
        .child(div().flex().items_center().child(app_menu_bar))
}
