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

    let app_state = AppState::new(cwd, config);

    tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_clipboard_manager::init())
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
            commands::search::start_search,
            commands::search::refresh_file_cache,
            commands::search::get_cwd,
            commands::preview::preview_file,
            commands::action::open_in_editor,
        ])
        .setup(|app| {
            use tauri::Manager;
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_always_on_top(true);
                let _ = window.set_visible_on_all_workspaces(true);
                let _ = window.set_focus();
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running clap-gui");
}
