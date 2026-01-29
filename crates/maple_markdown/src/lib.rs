//! Markdown preview server for vim-clap.
//!
//! This crate provides WebSocket-based markdown preview functionality
//! for the vim-clap plugin. It uses the `markdown_preview_core` library
//! for rendering, statistics, and file watching.

// Re-export core library types for convenience
pub use markdown_preview_core::{
    calculate_document_stats, find_git_root, rewrite_image_paths, to_html, toc, DocumentStats,
    RenderOptions, RenderResult,
};

use axum::extract::ws::{Message as WsMessage, WebSocket, WebSocketUpgrade};
use axum::extract::{Extension, Path as AxumPath, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{Html, IntoResponse};
use axum::routing::get;
use axum::Router;
use markdown_preview_core::assets::Assets;
use notify::{Event as NotifyEvent, RecommendedWatcher, RecursiveMode, Watcher};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, RwLock};
use tokio::sync::watch::Receiver;

type Error = Box<dyn std::error::Error + Send + Sync>;

/// Shared state for the markdown preview server.
#[derive(Clone)]
struct AppState {
    /// The directory containing the current markdown file.
    /// Used for resolving relative image paths.
    base_dir: Arc<RwLock<Option<PathBuf>>>,
}

/// Handler for serving static files (images, etc.) relative to the markdown file's directory.
async fn static_file_handler(
    State(state): State<AppState>,
    AxumPath(path): AxumPath<String>,
) -> impl IntoResponse {
    let base_dir = state.base_dir.read().unwrap().clone();

    let Some(base_dir) = base_dir else {
        return (StatusCode::NOT_FOUND, HeaderMap::new(), Vec::new());
    };

    // Decode URL-encoded path
    let decoded_path = percent_encoding::percent_decode_str(&path)
        .decode_utf8_lossy()
        .to_string();

    // Construct absolute path
    let file_path = base_dir.join(&decoded_path);

    // Security: ensure the resolved path is still within the base directory
    let canonical_base = match base_dir.canonicalize() {
        Ok(p) => p,
        Err(_) => return (StatusCode::NOT_FOUND, HeaderMap::new(), Vec::new()),
    };

    let canonical_file = match file_path.canonicalize() {
        Ok(p) => p,
        Err(_) => return (StatusCode::NOT_FOUND, HeaderMap::new(), Vec::new()),
    };

    if !canonical_file.starts_with(&canonical_base) {
        tracing::warn!(
            requested = %path,
            resolved = ?canonical_file,
            "Attempted path traversal attack"
        );
        return (StatusCode::FORBIDDEN, HeaderMap::new(), Vec::new());
    }

    // Read the file
    let content = match std::fs::read(&canonical_file) {
        Ok(c) => c,
        Err(_) => return (StatusCode::NOT_FOUND, HeaderMap::new(), Vec::new()),
    };

    // Determine content type based on extension
    let content_type = match canonical_file.extension().and_then(|e| e.to_str()) {
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("svg") => "image/svg+xml",
        Some("webp") => "image/webp",
        Some("ico") => "image/x-icon",
        Some("bmp") => "image/bmp",
        Some("pdf") => "application/pdf",
        _ => "application/octet-stream",
    };

    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, content_type.parse().unwrap());
    headers.insert(
        header::CACHE_CONTROL,
        "public, max-age=3600".parse().unwrap(),
    );

    (StatusCode::OK, headers, content)
}

/// The handler for the HTTP request (this gets called when the HTTP GET lands at the start
/// of websocket negotiation). After this completes, the actual switching from HTTP to
/// websocket protocol will occur.
async fn ws_handler(
    ws: Option<WebSocketUpgrade>,
    Extension(msg_rx): Extension<Receiver<Message>>,
    Extension(watcher_rx): Extension<Option<Receiver<Message>>>,
    Extension(disconnect_tx): Extension<Arc<Mutex<Option<tokio::sync::oneshot::Sender<()>>>>>,
    Extension(base_dir): Extension<Arc<RwLock<Option<PathBuf>>>>,
) -> impl IntoResponse {
    if let Some(ws) = ws {
        ws.on_upgrade(|ws| async move {
            handle_websocket(ws, msg_rx, watcher_rx, disconnect_tx, base_dir).await
        })
    } else {
        let html = Assets::build_html(&markdown_preview_core::assets::AssetOptions::default());
        let mut headers = HeaderMap::new();
        headers.insert(
            header::CACHE_CONTROL,
            "no-cache, no-store, must-revalidate".parse().unwrap(),
        );
        (StatusCode::OK, headers, Html(html)).into_response()
    }
}

async fn handle_websocket(
    mut socket: WebSocket,
    mut vim_rx: Receiver<Message>,
    mut watcher_rx: Option<Receiver<Message>>,
    disconnect_tx: Arc<Mutex<Option<tokio::sync::oneshot::Sender<()>>>>,
    base_dir: Arc<RwLock<Option<PathBuf>>>,
) {
    // Send initial message immediately when browser connects
    {
        let msg = vim_rx.borrow().clone();
        match process_message(msg) {
            Ok(text) => {
                if socket
                    .send(WsMessage::Text(text.to_string()))
                    .await
                    .is_err()
                {
                    tracing::error!("Failed to send initial message to browser");
                    return;
                }
                tracing::debug!("Sent initial message to browser");
            }
            Err(err) => {
                tracing::error!(?err, "Failed to process initial message");
            }
        }
    }

    loop {
        tokio::select! {
            // Messages FROM Vim TO browser
            result = vim_rx.changed() => {
                if result.is_err() {
                    break;
                }
                let msg = vim_rx.borrow().clone();
                let Ok(text) = process_message(msg) else {
                    break;
                };
                if socket.send(WsMessage::Text(text.to_string())).await.is_err() {
                    break;
                }
            }
            // Messages FROM file watcher TO browser
            result = async {
                match &mut watcher_rx {
                    Some(rx) => rx.changed().await,
                    None => std::future::pending().await,
                }
            } => {
                if result.is_err() {
                    tracing::warn!("File watcher channel closed, disabling auto-reload on external changes");
                    watcher_rx = None;
                    continue;
                }
                if let Some(rx) = &watcher_rx {
                    tracing::debug!("File watcher notified WebSocket handler, processing message");
                    let msg = rx.borrow().clone();
                    let Ok(text) = process_message(msg) else {
                        tracing::error!("Failed to process file watcher message");
                        continue;
                    };
                    tracing::debug!("Sending update to browser via WebSocket");
                    if socket.send(WsMessage::Text(text.to_string())).await.is_err() {
                        tracing::error!("Failed to send WebSocket message to browser");
                        break;
                    }
                    tracing::debug!("Successfully sent update to browser, ready for next change");
                }
            }
            // Messages FROM browser (detect disconnect or switch file requests)
            msg = socket.recv() => {
                match msg {
                    Some(Ok(WsMessage::Close(_))) | None => {
                        tracing::debug!("Browser disconnected (close frame or connection closed)");
                        break;
                    }
                    Some(Ok(WsMessage::Ping(data))) => {
                        if socket.send(WsMessage::Pong(data)).await.is_err() {
                            tracing::debug!("Failed to send pong, connection likely closed");
                            break;
                        }
                    }
                    Some(Ok(WsMessage::Text(text))) => {
                        if let Ok(request) = serde_json::from_str::<serde_json::Value>(&text) {
                            if request["type"] == "switch_file" {
                                if let Some(file_path) = request["file_path"].as_str() {
                                    tracing::info!(file_path, "Browser requested file switch");

                                    if let Some(parent) = Path::new(file_path).parent() {
                                        if let Ok(mut dir) = base_dir.write() {
                                            *dir = Some(parent.to_path_buf());
                                            tracing::debug!(new_base_dir = ?parent, "Updated base directory for image paths");
                                        }
                                    }

                                    let msg = Message::FileChanged(file_path.to_string(), false);
                                    let Ok(response) = process_message(msg) else {
                                        tracing::error!("Failed to process file switch request");
                                        continue;
                                    };
                                    if socket.send(WsMessage::Text(response.to_string())).await.is_err() {
                                        tracing::error!("Failed to send file switch response");
                                        break;
                                    }
                                }
                            }
                        }
                    }
                    Some(Err(err)) => {
                        tracing::debug!(?err, "WebSocket error, client likely disconnected");
                        break;
                    }
                    _ => {}
                }
            }
        }
    }

    tracing::debug!("WebSocket connection closed");

    if let Ok(mut guard) = disconnect_tx.lock() {
        if let Some(tx) = guard.take() {
            let _ = tx.send(());
            tracing::debug!("Sent disconnect notification");
        }
    }

    let _ = socket.send(WsMessage::Close(None)).await;
}

fn process_message(msg: Message) -> Result<serde_json::Value, Error> {
    let res = match msg {
        Message::FileChanged(path, should_focus) => {
            let markdown_content = std::fs::read_to_string(&path)?;
            let result = to_html(&markdown_content, &RenderOptions::gfm())?;
            let html = rewrite_image_paths(&result.html, "/files");
            let stats = calculate_document_stats(&markdown_content);
            let git_root = find_git_root(&path);

            serde_json::json!({
              "type": "update_content",
              "data": html,
              "source_lines": stats.lines,
              "line_map": result.line_map,
              "file_path": path,
              "git_root": git_root,
              "should_focus": should_focus,
              "stats": stats,
            })
        }
        Message::UpdateContent(content) => {
            serde_json::json!({
              "type": "update_content",
              "data": content,
              "source_lines": null,
            })
        }
        Message::Scroll(position) => {
            serde_json::json!({
              "type": "scroll",
              "data": position,
            })
        }
        Message::FocusWindow => {
            serde_json::json!({
              "type": "focus_window",
            })
        }
    };
    Ok(res)
}

/// Worker message that the websocket server deals with.
#[derive(Debug, Clone)]
pub enum Message {
    /// Markdown file was modified.
    /// The boolean flag indicates whether to focus the browser window.
    FileChanged(String, bool),
    /// Refresh the page with given HTML content.
    UpdateContent(String),
    /// Scroll to the given position specified in a percent to the window height.
    Scroll(usize),
    /// Request the browser window to focus itself.
    FocusWindow,
}

/// Spawns a polling-based file watcher as a fallback when inotify fails.
fn spawn_polling_file_watcher(
    file_path: String,
) -> (Receiver<Message>, std::sync::mpsc::Sender<()>) {
    let (msg_tx, msg_rx) = tokio::sync::watch::channel(Message::UpdateContent(String::new()));
    let (shutdown_tx, shutdown_rx) = std::sync::mpsc::channel();

    tokio::spawn(async move {
        let path = std::path::Path::new(&file_path);
        let mut last_mtime = std::fs::metadata(path).and_then(|m| m.modified()).ok();

        tracing::info!(
            path = %file_path,
            "Started polling-based file watcher (checking every second)"
        );

        loop {
            if shutdown_rx.try_recv().is_ok() {
                tracing::debug!("Polling file watcher shutting down");
                break;
            }

            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

            if let Ok(metadata) = std::fs::metadata(path) {
                if let Ok(current_mtime) = metadata.modified() {
                    if let Some(last) = last_mtime {
                        if current_mtime > last {
                            tracing::debug!(path = %file_path, "File modified, sending update");
                            msg_tx.send_replace(Message::FileChanged(file_path.clone(), false));
                            last_mtime = Some(current_mtime);
                        }
                    } else {
                        last_mtime = Some(current_mtime);
                    }
                }
            }
        }

        tracing::debug!("Polling file watcher task exited");
    });

    (msg_rx, shutdown_tx)
}

/// Spawns a file watcher that monitors changes to the given file.
fn spawn_file_watcher(
    file_path: String,
) -> Result<(Receiver<Message>, std::sync::mpsc::Sender<()>), Error> {
    let (msg_tx, msg_rx) = tokio::sync::watch::channel(Message::UpdateContent(String::new()));
    let (event_tx, event_rx) = std::sync::mpsc::channel();
    let (shutdown_tx, shutdown_rx) = std::sync::mpsc::channel();
    let (started_tx, started_rx) = std::sync::mpsc::channel();

    let file_path_for_async = file_path.clone();
    let file_path_for_thread = file_path.clone();

    let watch_path = Path::new(&file_path_for_thread);
    let (watch_target, file_name) = if let (Some(parent), Some(name)) =
        (watch_path.parent(), watch_path.file_name())
    {
        (parent.to_path_buf(), name.to_os_string())
    } else {
        tracing::error!(path = ?file_path_for_thread, "Invalid file path - cannot determine parent directory");
        return Err("Invalid file path".into());
    };

    let file_name_for_filter = file_name.clone();

    let shutdown_rx_clone = shutdown_rx;
    std::thread::spawn(move || {
        let mut watcher = match RecommendedWatcher::new(
            move |res: Result<NotifyEvent, notify::Error>| match res {
                Ok(event) => {
                    tracing::debug!(?event, target_file = ?file_name_for_filter, "File watcher received event");

                    let is_target_file = event
                        .paths
                        .iter()
                        .any(|p| p.file_name() == Some(&file_name_for_filter));

                    if !is_target_file {
                        tracing::debug!(?event.paths, "Event not for target file, ignoring");
                        return;
                    }

                    if event.kind.is_modify()
                        || event.kind.is_create()
                        || event.kind.is_remove()
                        || event.kind.is_access()
                    {
                        tracing::debug!(kind = ?event.kind, "File change detected, sending notification");
                        match event_tx.send(()) {
                            Ok(()) => {
                                tracing::debug!("Notification sent successfully to bridge task")
                            }
                            Err(e) => tracing::error!(
                                ?e,
                                "Failed to send notification - bridge task may have exited"
                            ),
                        }
                    } else {
                        tracing::debug!(kind = ?event.kind, "Ignoring event type");
                    }
                }
                Err(e) => {
                    tracing::error!(?e, "File watcher error");
                }
            },
            notify::Config::default(),
        ) {
            Ok(w) => w,
            Err(err) => {
                tracing::error!(?err, "Failed to create file watcher");
                return;
            }
        };

        if let Err(err) = watcher.watch(&watch_target, RecursiveMode::NonRecursive) {
            tracing::error!(?err, path = ?watch_target, "Failed to watch directory");
            let _ = started_tx.send(Err(err.to_string()));
            return;
        }

        let _ = started_tx.send(Ok(()));

        tracing::debug!(
            watch_dir = ?watch_target,
            target_file = ?file_name,
            "File watcher started on parent directory"
        );

        loop {
            match shutdown_rx_clone.recv_timeout(std::time::Duration::from_secs(1)) {
                Ok(_) | Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                    tracing::debug!(path = ?file_path_for_thread, "File watcher shutting down");
                    break;
                }
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
            }
        }
    });

    tokio::task::spawn_blocking(move || {
        while let Ok(()) = event_rx.recv() {
            tracing::debug!(path = ?file_path_for_async, "File changed detected by watcher, bridging to async channel");
            let receiver_count = msg_tx.receiver_count();
            tracing::debug!(receiver_count, "Current receiver count");
            if receiver_count == 0 {
                tracing::debug!("No receivers left, exiting bridge task");
                break;
            }
            msg_tx.send_replace(Message::FileChanged(file_path_for_async.clone(), false));
            tracing::debug!("Message sent via send_replace, waiting for next file event");
        }
        tracing::debug!("File watcher bridge task exiting - event_rx closed");
    });

    match started_rx.recv_timeout(std::time::Duration::from_secs(2)) {
        Ok(Ok(())) => {
            tracing::debug!("File watcher started successfully");
            Ok((msg_rx, shutdown_tx))
        }
        Ok(Err(err_msg)) => {
            tracing::error!(error = %err_msg, "File watcher failed to start");
            Err(err_msg.into())
        }
        Err(_) => {
            tracing::error!("Timeout waiting for file watcher to start");
            Err("Watcher startup timeout".into())
        }
    }
}

/// Configuration for opening a markdown preview in the browser.
pub struct PreviewConfig {
    /// TCP listener for the web server.
    pub listener: tokio::net::TcpListener,
    /// Receiver for messages from Vim.
    pub msg_rx: Receiver<Message>,
    /// Receiver for graceful shutdown signal.
    pub shutdown_rx: tokio::sync::oneshot::Receiver<()>,
    /// Optional file path to watch for changes.
    pub file_path: Option<String>,
    /// Optional sender to notify when browser disconnects.
    pub disconnect_tx: Option<tokio::sync::oneshot::Sender<()>>,
}

pub async fn open_preview_in_browser(config: PreviewConfig) -> Result<(), Error> {
    let PreviewConfig {
        listener,
        msg_rx,
        shutdown_rx,
        file_path,
        disconnect_tx,
    } = config;

    let (watcher_rx, _watcher_shutdown) = if let Some(ref path) = file_path {
        match spawn_file_watcher(path.clone()) {
            Ok((watcher_rx, shutdown)) => {
                tracing::info!("Started inotify-based file watcher");
                (Some(watcher_rx), Some(shutdown))
            }
            Err(err) => {
                tracing::warn!(
                    ?err,
                    "inotify file watcher failed, falling back to polling (checks every 1 second)"
                );
                let (polling_rx, shutdown) = spawn_polling_file_watcher(path.clone());
                (Some(polling_rx), Some(shutdown))
            }
        }
    } else {
        (None, None)
    };

    let disconnect_tx_shared = Arc::new(Mutex::new(disconnect_tx));

    let base_dir = file_path
        .as_ref()
        .and_then(|p| Path::new(p).parent().map(|parent| parent.to_path_buf()));
    let app_state = AppState {
        base_dir: Arc::new(RwLock::new(base_dir)),
    };

    let app = Router::new()
        .route("/", get(ws_handler))
        .route("/files/*path", get(static_file_handler))
        .layer(Extension(msg_rx))
        .layer(Extension(watcher_rx))
        .layer(Extension(disconnect_tx_shared))
        .layer(Extension(app_state.base_dir.clone()))
        .with_state(app_state);

    let port = listener.local_addr()?.port();

    webbrowser::open(&format!("http://127.0.0.1:{port}"))?;

    tracing::debug!("Listening on {listener:?}");

    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(async move {
        let _ = shutdown_rx.await;
        tracing::debug!("Received shutdown signal for preview server");
    })
    .await?;

    tracing::debug!("Preview server shutting down");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore]
    async fn it_works() {
        let (msg_tx, msg_rx) = tokio::sync::watch::channel(Message::UpdateContent(String::new()));

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(1));
            for _i in 0..10 {
                interval.tick().await;
                let html = format!("Current time: {:?}", std::time::Instant::now());
                msg_tx.send_replace(Message::UpdateContent(html));
            }
        });

        let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
            .await
            .unwrap();

        let (_shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();

        open_preview_in_browser(PreviewConfig {
            listener,
            msg_rx,
            shutdown_rx,
            file_path: None,
            disconnect_tx: None,
        })
        .await
        .expect("Failed to open markdown preview");
    }
}
