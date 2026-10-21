//! The macOS menu bar at the top of the screen (GrAMF / File / Edit / View / Window). Its
//! items become [`Command`]s; elsewhere the in-window egui menus do this job.

use muda::accelerator::Accelerator;
use muda::{AboutMetadata, CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu};

pub enum Command {
    Open,
    Save,
    Export,
    Undo,
    Redo,
    ResetCamera,
    ZoomIn,
    ZoomOut,
    ZoomSelection,
    Developer,
    Settings,
    Quit,
}

pub struct NativeMenu {
    /// Kept alive: dropping it would tear the menu bar down.
    _menu: Menu,
    save: MenuItem,
    export: MenuItem,
    developer: CheckMenuItem,
    /// Last `(store open, developer view)` pushed to the items.
    synced: Option<(bool, bool)>,
}

fn accel(s: &str) -> Option<Accelerator> {
    s.parse().ok()
}

impl NativeMenu {
    /// Builds the menu bar and hands it to NSApp; call once the event loop is running.
    pub fn install() -> Option<Self> {
        let item = |id: &str, text: &str, keys: &str| MenuItem::with_id(id, text, true, accel(keys));
        let save = item("save", "Save Annotations", "CmdOrCtrl+S");
        let export = item("export", "Export Tables…", "CmdOrCtrl+E");
        let developer = CheckMenuItem::with_id("developer", "Developer View", true, false, None);
        let about = AboutMetadata {
            name: Some("GrAMF".into()),
            version: Some(env!("CARGO_PKG_VERSION").into()),
            comments: Some("Graphs for Arbuscular Mycorrhizal Fungi".into()),
            ..Default::default()
        };
        let sep = PredefinedMenuItem::separator;

        let app = Submenu::with_items(
            "GrAMF",
            true,
            &[
                &PredefinedMenuItem::about(None, Some(about)),
                &sep(),
                &item("settings", "Settings…", "CmdOrCtrl+Comma"),
                &sep(),
                &PredefinedMenuItem::services(None),
                &sep(),
                &PredefinedMenuItem::hide(None),
                &PredefinedMenuItem::hide_others(None),
                &PredefinedMenuItem::show_all(None),
                &sep(),
                // our own Quit, so unsaved annotations get the save / discard prompt
                &item("quit", "Quit GrAMF", "CmdOrCtrl+Q"),
            ],
        )
        .ok()?;
        let file =
            Submenu::with_items("File", true, &[&item("open", "Open Store…", "CmdOrCtrl+O"), &sep(), &save, &export]).ok()?;
        // Undo/Redo act on annotations; copy and paste stay with egui's text fields.
        let edit = Submenu::with_items(
            "Edit",
            true,
            &[&item("undo", "Undo", "CmdOrCtrl+Z"), &item("redo", "Redo", "CmdOrCtrl+Shift+Z")],
        )
        .ok()?;
        // No Home key equivalent: it would steal Home from text fields; the map handles Home.
        let view = Submenu::with_items(
            "View",
            true,
            &[
                &item("zoom_in", "Zoom In", "CmdOrCtrl+Equal"),
                &item("zoom_out", "Zoom Out", "CmdOrCtrl+Minus"),
                &item("reset_camera", "Reset Camera", "CmdOrCtrl+0"),
                &item("zoom_selection", "Zoom to Selection", "CmdOrCtrl+2"),
                &sep(),
                &developer,
            ],
        )
        .ok()?;
        let window = Submenu::with_items(
            "Window",
            true,
            &[&PredefinedMenuItem::minimize(None), &PredefinedMenuItem::fullscreen(None)],
        )
        .ok()?;

        let menu = Menu::with_items(&[&app, &file, &edit, &view, &window]).ok()?;
        menu.init_for_nsapp();
        window.set_as_windows_menu_for_nsapp();
        Some(Self { _menu: menu, save, export, developer, synced: None })
    }

    /// Commands chosen since the last call.
    pub fn poll() -> Vec<Command> {
        MenuEvent::receiver()
            .try_iter()
            .filter_map(|event| match event.id.as_ref() {
                "open" => Some(Command::Open),
                "save" => Some(Command::Save),
                "export" => Some(Command::Export),
                "undo" => Some(Command::Undo),
                "redo" => Some(Command::Redo),
                "reset_camera" => Some(Command::ResetCamera),
                "zoom_in" => Some(Command::ZoomIn),
                "zoom_out" => Some(Command::ZoomOut),
                "zoom_selection" => Some(Command::ZoomSelection),
                "developer" => Some(Command::Developer),
                "settings" => Some(Command::Settings),
                "quit" => Some(Command::Quit),
                _ => None,
            })
            .collect()
    }

    /// Greys out store-only items and mirrors the developer-view toggle.
    pub fn sync(&mut self, store_open: bool, developer: bool) {
        if self.synced != Some((store_open, developer)) {
            self.save.set_enabled(store_open);
            self.export.set_enabled(store_open);
            self.developer.set_checked(developer);
            self.synced = Some((store_open, developer));
        }
    }
}
