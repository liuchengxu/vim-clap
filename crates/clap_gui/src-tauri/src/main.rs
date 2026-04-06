#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod config;
mod state;

use state::AppState;
use std::path::PathBuf;
use std::process::Command;

/// Detect the git repository root from the current directory.
fn git_repo_root() -> Option<PathBuf> {
    Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| {
            String::from_utf8(o.stdout)
                .ok()
                .map(|s| PathBuf::from(s.trim()))
        })
}

fn main() {
    let config = config::load_config();

    let args: Vec<String> = std::env::args().collect();
    let cwd = args
        .get(1)
        .filter(|a| !a.starts_with('-'))
        .map(PathBuf::from)
        .unwrap_or_else(|| git_repo_root().unwrap_or_else(|| {
            std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
        }));

    let hotkey = config.window.hotkey.clone();
    let app_state = AppState::new(cwd, config);

    tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
            commands::search::start_search,
            commands::search::refresh_file_cache,
            commands::search::get_cwd,
            commands::search::quit_app,
            commands::preview::preview_file,
            commands::action::open_in_editor,
        ])
        .setup(move |app| {
            use tauri::Manager;
            use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut};

            if let Some(window) = app.get_webview_window("main") {
                // Resize window proportionally to the monitor size.
                if let Some(monitor) = window.current_monitor().ok().flatten() {
                    let screen = monitor.size();
                    let scale = monitor.scale_factor();
                    let screen_w = screen.width as f64 / scale;
                    let screen_h = screen.height as f64 / scale;

                    let w = (screen_w * 0.6).round();
                    let h = (screen_h * 0.55).round();

                    let _ = window.set_size(tauri::LogicalSize::new(w, h));
                    let x = ((screen_w - w) / 2.0).round();
                    let y = ((screen_h - h) / 2.0).round();
                    let _ = window.set_position(tauri::LogicalPosition::new(x, y));
                }

                let _ = window.set_always_on_top(true);
                let _ = window.set_visible_on_all_workspaces(true);
                let _ = window.set_focus();

                // Register global hotkey to toggle window visibility.
                if let Ok(shortcut) = hotkey.parse::<Shortcut>() {
                    let win = window.clone();
                    let _ = app.global_shortcut().on_shortcut(shortcut, move |_app, _shortcut, _event| {
                        if win.is_visible().unwrap_or(false) {
                            let _ = win.hide();
                        } else {
                            let _ = win.show();
                            let _ = win.set_focus();
                        }
                    });
                }
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running clap-gui");
}
