//! Standalone markdown preview application using Tauri.
//!
//! This application provides a native desktop app for previewing markdown files
//! with the same features as the vim-clap integration: GitHub Flavored Markdown,
//! syntax highlighting, Mermaid diagrams, KaTeX math, and themes.
//!
//! # Usage
//!
//! ```bash
//! # Open without a file (use File > Open or Cmd+O)
//! markdown_preview_app
//!
//! # Open with a specific file
//! markdown_preview_app /path/to/file.md
//! ```

// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod menu;
mod state;

use markdown_preview_core::DocumentType;
use state::AppState;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::{Emitter, Manager};
use tokio::sync::RwLock;

fn main() {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("markdown_preview_app=debug".parse().unwrap())
                .add_directive("markdown_preview_core=debug".parse().unwrap()),
        )
        .init();

    tracing::info!("Starting Markdown Preview App");

    // Parse command line arguments for initial file
    let initial_file = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .filter(|p| p.exists() && is_supported_file(p));

    if let Some(ref path) = initial_file {
        tracing::info!(path = %path.display(), "Opening file from command line");
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_opener::init())
        .setup(move |app| {
            // Get app data directory for persistent config
            let config_dir = app.path().app_data_dir().ok();
            if let Some(ref dir) = config_dir {
                tracing::info!(path = %dir.display(), "Using config directory");
            }

            // Initialize state with config directory for persistence
            let state = AppState::new(config_dir);
            app.manage(Arc::new(RwLock::new(state)));

            // Set up the menu
            let menu = menu::create_menu(app.handle())?;
            app.set_menu(menu)?;

            // Handle menu events
            app.on_menu_event(move |app, event| {
                menu::handle_menu_event(app, &event);
            });

            // If we have an initial file from command line, open it after the window is ready
            if let Some(path) = initial_file.clone() {
                let handle = app.handle().clone();
                let path_str = path.to_string_lossy().to_string();

                // Spawn a task to open the file once the window is ready
                tauri::async_runtime::spawn(async move {
                    // Small delay to ensure the window is fully loaded
                    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

                    if let Some(window) = handle.get_webview_window("main") {
                        // Emit event to frontend to open the file
                        let _ = window.emit("open-initial-file", &path_str);
                    }
                });
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::render_markdown,
            commands::open_file,
            commands::get_recent_files,
            commands::add_recent_file,
            commands::clear_recent_files,
            commands::watch_file,
            commands::unwatch_file,
            commands::check_clipboard_for_markdown,
            commands::complete_path,
            commands::open_url,
            commands::get_path_history,
            commands::add_path_to_history,
            commands::get_current_git_root,
            commands::refresh_file_metadata,
            commands::get_supported_extensions,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// Check if a path is a supported document file.
fn is_supported_file(path: &std::path::Path) -> bool {
    DocumentType::from_path(path).is_some()
}
