//! Native menu implementation for the markdown preview app.

use tauri::menu::{Menu, MenuBuilder, MenuEvent, MenuItem, SubmenuBuilder};
use tauri::{AppHandle, Emitter, Manager, Runtime};

/// Create the application menu.
pub fn create_menu<R: Runtime>(app: &AppHandle<R>) -> Result<Menu<R>, tauri::Error> {
    // File menu
    let open_item = MenuItem::with_id(app, "open", "Open...", true, Some("CmdOrCtrl+O"))?;
    let close_item = MenuItem::with_id(app, "close", "Close", true, Some("CmdOrCtrl+W"))?;

    let file_menu = SubmenuBuilder::new(app, "File")
        .item(&open_item)
        .separator()
        .item(&close_item)
        .build()?;

    // View menu
    let reload_item = MenuItem::with_id(app, "reload", "Reload", true, Some("CmdOrCtrl+R"))?;
    let toggle_toc_item =
        MenuItem::with_id(app, "toggle_toc", "Toggle Table of Contents", true, Some("CmdOrCtrl+T"))?;

    let view_menu = SubmenuBuilder::new(app, "View")
        .item(&reload_item)
        .separator()
        .item(&toggle_toc_item)
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
    let about_item = MenuItem::with_id(app, "about", "About Markdown Preview", true, None::<&str>)?;

    let help_menu = SubmenuBuilder::new(app, "Help")
        .item(&about_item)
        .build()?;

    // Build the complete menu
    MenuBuilder::new(app)
        .item(&file_menu)
        .item(&view_menu)
        .item(&theme_menu)
        .item(&help_menu)
        .build()
}

/// Handle menu events.
pub fn handle_menu_event<R: Runtime>(app: &AppHandle<R>, event: &MenuEvent) {
    match event.id().as_ref() {
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
        "toggle_toc" => {
            // Emit event to frontend to toggle TOC
            let _ = app.emit("menu-toggle-toc", ());
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
        "about" => {
            // Show about dialog
            let _ = app.emit("menu-about", ());
        }
        _ => {
            tracing::debug!(id = event.id().as_ref(), "Unhandled menu event");
        }
    }
}
