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

mod ai;
mod commands;
mod menu;
mod state;

use futures::stream::StreamExt;
use markdown_preview_core::DocumentType;
use state::AppState;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
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
            let mut state = AppState::new(config_dir);

            // Set up background snapshot writer for non-blocking, ordered persistence
            let (snap_tx, mut snap_rx) =
                tokio::sync::mpsc::unbounded_channel::<(std::path::PathBuf, String)>();
            state.set_snapshot_writer(snap_tx);
            tauri::async_runtime::spawn(async move {
                while let Some((path, content)) = snap_rx.recv().await {
                    // Write on blocking thread, await completion before processing next
                    let _ = tokio::task::spawn_blocking(move || {
                        if let Some(parent) = path.parent() {
                            let _ = std::fs::create_dir_all(parent);
                        }
                        if let Err(e) = std::fs::write(&path, &content) {
                            tracing::warn!(error = %e, "Failed to write snapshots file");
                        } else {
                            tracing::debug!(path = %path.display(), "Saved file snapshots");
                        }
                    })
                    .await;
                }
            });

            app.manage(Arc::new(RwLock::new(state)));
            app.manage(commands::terminal::TerminalState::default());

            // Set up the menu
            let menu = menu::create_menu(app.handle())?;
            app.set_menu(menu)?;

            // Handle menu events
            app.on_menu_event(move |app, event| {
                menu::handle_menu_event(app, &event);
            });

            // Spawn background AI summarization for recent files
            {
                let state_arc: Arc<RwLock<AppState>> =
                    app.state::<Arc<RwLock<AppState>>>().inner().clone();
                let handle = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    generate_recent_file_summaries(state_arc, handle).await;
                });
            }

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
            commands::render::render_markdown,
            commands::file::open_file,
            commands::file::watch_file,
            commands::file::unwatch_file,
            commands::recent::get_recent_files,
            commands::recent::add_recent_file,
            commands::recent::clear_recent_files,
            commands::recent::remove_recent_file,
            commands::clipboard::check_clipboard_for_markdown,
            commands::path::complete_path,
            commands::path::get_path_history,
            commands::path::add_path_to_history,
            commands::git::get_current_git_root,
            commands::url::open_url,
            commands::url::open_url_with_token,
            commands::metadata::refresh_file_metadata,
            commands::metadata::get_supported_extensions,
            commands::metadata::get_markdown_title,
            commands::metadata::get_file_preview_info,
            commands::metadata::get_ai_config,
            commands::metadata::set_ai_config,
            commands::metadata::get_app_config,
            commands::metadata::set_app_config,
            commands::diff::get_file_diff,
            commands::terminal::spawn_terminal,
            commands::terminal::write_terminal,
            commands::terminal::resize_terminal,
            commands::terminal::kill_terminal,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// Check if a path is a supported document file.
fn is_supported_file(path: &std::path::Path) -> bool {
    DocumentType::from_path(path).is_some()
}

/// Get the file modification time in Unix millis.
async fn get_file_mtime(path: &std::path::Path) -> Option<u64> {
    tokio::fs::metadata(path)
        .await
        .ok()
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as u64)
}

/// Background task: generate AI summaries for recent files that are missing or stale.
async fn generate_recent_file_summaries(
    state: Arc<RwLock<AppState>>,
    app_handle: tauri::AppHandle,
) {
    // Small delay to let the app finish initializing
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    // Read config and recent files list
    let (ai_config, recent_files) = {
        let state_guard = state.read().await;
        let config = ai::AiConfig::from_state(
            state_guard.ai_provider(),
            state_guard.ai_model(),
            state_guard.ai_api_key(),
            state_guard.ollama_url(),
        );
        let files = state_guard.get_recent_files();
        (config, files)
    };

    if !ai_config.is_enabled() {
        tracing::debug!("AI summarization not configured, skipping startup summaries");
        return;
    }

    // Pre-scan: collect files that actually need summarization
    let mut eligible_files: Vec<(String, u64)> = Vec::new();
    {
        let state_guard = state.read().await;
        for file_path in &recent_files {
            let path = std::path::Path::new(file_path);
            if DocumentType::from_path(path) != Some(DocumentType::Markdown) {
                continue;
            }
            if let Some(mtime) = get_file_mtime(path).await {
                if state_guard.get_ai_summary(file_path, mtime).is_none() {
                    eligible_files.push((file_path.clone(), mtime));
                }
            }
        }
    }

    let total = eligible_files.len();
    if total == 0 {
        tracing::debug!("All recent files already have fresh summaries");
        return;
    }

    tracing::info!(
        provider = ?ai_config.provider,
        total,
        "Starting background AI summarization for recent files"
    );

    let _ = app_handle.emit(
        "ai-summary-progress",
        serde_json::json!({ "status": "batch_started", "total": total }),
    );

    let concurrency = ai_config.provider.max_concurrency();
    let completed = Arc::new(AtomicUsize::new(0));
    let ai_config = Arc::new(ai_config);

    futures::stream::iter(eligible_files)
        .map(|(file_path, mtime)| {
            let state = Arc::clone(&state);
            let app_handle = app_handle.clone();
            let completed = Arc::clone(&completed);
            let ai_config = Arc::clone(&ai_config);

            async move {
                let path = std::path::Path::new(&file_path);

                let content = match tokio::fs::read_to_string(path).await {
                    Ok(content) => content,
                    Err(_) => return,
                };

                if let Some(summary) = ai::summarize(&ai_config, &content).await {
                    let mut state_guard = state.write().await;
                    state_guard.set_ai_summary(file_path.clone(), summary, mtime);
                }

                let done = completed.fetch_add(1, Ordering::Relaxed).saturating_add(1);
                let _ = app_handle.emit(
                    "ai-summary-progress",
                    serde_json::json!({
                        "status": "file_done",
                        "filePath": file_path,
                        "completed": done,
                        "total": total
                    }),
                );
            }
        })
        .buffer_unordered(concurrency)
        .collect::<()>()
        .await;

    let _ = app_handle.emit(
        "ai-summary-progress",
        serde_json::json!({ "status": "batch_done" }),
    );

    tracing::info!("Background AI summarization complete");
}

/// Spawn a background AI summary task for a single file.
///
/// Called from file open to ensure newly opened files get summarized.
pub fn spawn_summary_for_file(
    state: Arc<RwLock<AppState>>,
    file_path: String,
    app_handle: tauri::AppHandle,
) {
    tauri::async_runtime::spawn(async move {
        let path = std::path::Path::new(&file_path);

        if DocumentType::from_path(path) != Some(DocumentType::Markdown) {
            return;
        }

        let mtime = match get_file_mtime(path).await {
            Some(mtime) => mtime,
            None => return,
        };

        // Check config and cache
        let ai_config = {
            let state_guard = state.read().await;
            if state_guard.get_ai_summary(&file_path, mtime).is_some() {
                return;
            }
            ai::AiConfig::from_state(
                state_guard.ai_provider(),
                state_guard.ai_model(),
                state_guard.ai_api_key(),
                state_guard.ollama_url(),
            )
        };

        if !ai_config.is_enabled() {
            return;
        }

        let _ = app_handle.emit(
            "ai-summary-progress",
            serde_json::json!({ "status": "file_started", "filePath": &file_path }),
        );

        let content = match tokio::fs::read_to_string(path).await {
            Ok(content) => content,
            Err(_) => return,
        };

        if let Some(summary) = ai::summarize(&ai_config, &content).await {
            let mut state_guard = state.write().await;
            state_guard.set_ai_summary(file_path.clone(), summary, mtime);

            let _ = app_handle.emit(
                "ai-summary-progress",
                serde_json::json!({ "status": "file_done", "filePath": &file_path }),
            );
        }
    });
}
