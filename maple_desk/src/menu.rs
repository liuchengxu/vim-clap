//! Native menu implementation for the markdown preview app.

use tauri::menu::{Menu, MenuBuilder, MenuEvent, MenuItem, PredefinedMenuItem, SubmenuBuilder};
use tauri::{AppHandle, Emitter, Manager, Runtime};

/// Create the application menu.
pub fn create_menu<R: Runtime>(app: &AppHandle<R>) -> Result<Menu<R>, tauri::Error> {
    // File menu
    let open_path_item =
        MenuItem::with_id(app, "open_path", "Open Path...", true, Some("CmdOrCtrl+O"))?;
    let open_item =
        MenuItem::with_id(app, "open", "Open File...", true, Some("CmdOrCtrl+Shift+O"))?;
    let settings_item =
        MenuItem::with_id(app, "settings", "Settings...", true, Some("CmdOrCtrl+,"))?;
    let close_item = MenuItem::with_id(app, "close", "Close", true, Some("CmdOrCtrl+W"))?;

    let file_menu = SubmenuBuilder::new(app, "File")
        .item(&open_path_item)
        .item(&open_item)
        .separator()
        .item(&settings_item)
        .separator()
        .item(&close_item)
        .build()?;

    // Edit menu (required on macOS for Cmd+C/V/X/A/Z to work)
    let edit_menu = SubmenuBuilder::new(app, "Edit")
        .item(&PredefinedMenuItem::undo(app, None)?)
        .item(&PredefinedMenuItem::redo(app, None)?)
        .separator()
        .item(&PredefinedMenuItem::cut(app, None)?)
        .item(&PredefinedMenuItem::copy(app, None)?)
        .item(&PredefinedMenuItem::paste(app, None)?)
        .item(&PredefinedMenuItem::select_all(app, None)?)
        .build()?;

    // View menu
    let reload_item = MenuItem::with_id(app, "reload", "Reload", true, Some("CmdOrCtrl+R"))?;

    // TOC submenu
    let toc_off = MenuItem::with_id(app, "toc_off", "Off", true, None::<&str>)?;
    let toc_left = MenuItem::with_id(app, "toc_left", "Left", true, None::<&str>)?;
    let toc_right = MenuItem::with_id(app, "toc_right", "Right", true, None::<&str>)?;

    let toc_menu = SubmenuBuilder::new(app, "Table of Contents")
        .item(&toc_off)
        .item(&toc_left)
        .item(&toc_right)
        .build()?;

    let terminal_item = MenuItem::with_id(
        app,
        "toggle_terminal",
        "Toggle Terminal",
        true,
        Some("CmdOrCtrl+`"),
    )?;

    let dictionary_item =
        MenuItem::with_id(app, "dictionary", "Dictionary", true, None::<&str>)?;

    let view_menu = SubmenuBuilder::new(app, "View")
        .item(&reload_item)
        .item(&terminal_item)
        .item(&dictionary_item)
        .separator()
        .item(&toc_menu)
        .build()?;

    // Theme submenu
    let theme_light = MenuItem::with_id(app, "theme_light", "Light", true, None::<&str>)?;
    let theme_dark = MenuItem::with_id(app, "theme_dark", "Dark", true, None::<&str>)?;
    let theme_auto = MenuItem::with_id(app, "theme_auto", "Auto", true, None::<&str>)?;

    let theme_menu = SubmenuBuilder::new(app, "Theme")
        .item(&theme_light)
        .item(&theme_dark)
        .item(&theme_auto)
        .build()?;

    // Help menu
    let about_item = MenuItem::with_id(app, "about", "About Maple Desk", true, None::<&str>)?;

    let help_menu = SubmenuBuilder::new(app, "Help").item(&about_item).build()?;

    // Build the complete menu
    MenuBuilder::new(app)
        .item(&file_menu)
        .item(&edit_menu)
        .item(&view_menu)
        .item(&theme_menu)
        .item(&help_menu)
        .build()
}

/// Handle menu events.
pub fn handle_menu_event<R: Runtime>(app: &AppHandle<R>, event: &MenuEvent) {
    match event.id().as_ref() {
        "open_path" => {
            // Emit event to frontend to show path input modal
            let _ = app.emit("menu-open-path", ());
        }
        "open" => {
            // Emit event to frontend to open file dialog
            let _ = app.emit("menu-open", ());
        }
        "close" => {
            // Close the current window
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.close();
            }
        }
        "reload" => {
            // Emit event to frontend to reload current file
            let _ = app.emit("menu-reload", ());
        }
        "toc_off" => {
            let _ = app.emit("menu-toc", "off");
        }
        "toc_left" => {
            let _ = app.emit("menu-toc", "left");
        }
        "toc_right" => {
            let _ = app.emit("menu-toc", "right");
        }
        "theme_light" => {
            let _ = app.emit("menu-theme", "light");
        }
        "theme_dark" => {
            let _ = app.emit("menu-theme", "dark");
        }
        "theme_auto" => {
            let _ = app.emit("menu-theme", "auto");
        }
        "toggle_terminal" => {
            let _ = app.emit("menu-toggle-terminal", ());
        }
        "dictionary" => {
            let _ = app.emit("menu-dictionary", ());
        }
        "settings" => {
            let _ = app.emit("menu-settings", ());
        }
        "about" => {
            // Show about dialog
            let _ = app.emit("menu-about", ());
        }
        _ => {
            tracing::debug!(id = event.id().as_ref(), "Unhandled menu event");
        }
    }
}
