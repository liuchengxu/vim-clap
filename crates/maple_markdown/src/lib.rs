pub mod toc;

use axum::extract::ws::{Message as WsMessage, WebSocket, WebSocketUpgrade};
use axum::extract::Extension;
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse};
use axum::routing::get;
use axum::Router;
use notify::{Event as NotifyEvent, RecommendedWatcher, RecursiveMode, Watcher};
use std::net::SocketAddr;
use std::path::Path;
use std::sync::{Arc, Mutex};
use tokio::sync::watch::Receiver;

type Error = Box<dyn std::error::Error + Send + Sync>;

/// The handler for the HTTP request (this gets called when the HTTP GET lands at the start
/// of websocket negotiation). After this completes, the actual switching from HTTP to
/// websocket protocol will occur.
/// This is the last point where we can extract TCP/IP metadata such as IP address of the client
/// as well as things from HTTP headers such as user-agent of the browser etc.
async fn ws_handler(
    ws: Option<WebSocketUpgrade>,
    Extension(msg_rx): Extension<Receiver<Message>>,
    Extension(watcher_rx): Extension<Option<Receiver<Message>>>,
    Extension(disconnect_tx): Extension<Arc<Mutex<Option<tokio::sync::oneshot::Sender<()>>>>>,
) -> impl IntoResponse {
    if let Some(ws) = ws {
        ws.on_upgrade(
            |ws| async move { handle_websocket(ws, msg_rx, watcher_rx, disconnect_tx).await },
        )
    } else {
        let html = include_str!("../js/index.html");
        (StatusCode::OK, Html(html)).into_response()
    }
}

async fn handle_websocket(
    mut socket: WebSocket,
    mut vim_rx: Receiver<Message>,
    mut watcher_rx: Option<Receiver<Message>>,
    disconnect_tx: Arc<Mutex<Option<tokio::sync::oneshot::Sender<()>>>>,
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
                // Don't return here - keep the connection open for future updates
            }
        }
    }

    loop {
        // Wait for messages from Vim, file watcher, or browser
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
                    // File watcher failed - disable it but keep the WebSocket alive
                    watcher_rx = None;
                    continue;
                }
                if let Some(rx) = &watcher_rx {
                    tracing::debug!("File watcher notified WebSocket handler, processing message");
                    let msg = rx.borrow().clone();
                    let Ok(text) = process_message(msg) else {
                        tracing::error!("Failed to process file watcher message");
                        continue;  // Don't break, just skip this update
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
                        // Respond to keep-alive pings
                        if socket.send(WsMessage::Pong(data)).await.is_err() {
                            tracing::debug!("Failed to send pong, connection likely closed");
                            break;
                        }
                    }
                    Some(Ok(WsMessage::Text(text))) => {
                        // Handle messages from browser (e.g., switch file request)
                        if let Ok(request) = serde_json::from_str::<serde_json::Value>(&text) {
                            if request["type"] == "switch_file" {
                                if let Some(file_path) = request["file_path"].as_str() {
                                    tracing::info!(file_path, "Browser requested file switch");
                                    let msg = Message::FileChanged(file_path.to_string());
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
                    _ => {
                        // Ignore other message types (Binary, Pong)
                    }
                }
            }
        }
    }

    tracing::debug!("WebSocket connection closed");

    // Notify caller that browser disconnected
    if let Ok(mut guard) = disconnect_tx.lock() {
        if let Some(tx) = guard.take() {
            let _ = tx.send(());
            tracing::debug!("Sent disconnect notification");
        }
    }

    let _ = socket.send(WsMessage::Close(None)).await;
}

/// Detects GitHub alert type from blockquote content.
/// Returns (alert_type, title, svg_icon) if this is a GitHub alert, None otherwise.
///
/// SVG icons are from GitHub's official Octicons library:
/// https://github.com/primer/octicons
/// License: MIT (c) GitHub, Inc.
/// CDN source: https://unpkg.com/@primer/octicons/build/svg/
fn detect_github_alert(text: &str) -> Option<(&'static str, &'static str, &'static str)> {
    let trimmed = text.trim();

    // GitHub alert patterns: [!NOTE], [!TIP], [!IMPORTANT], [!WARNING], [!CAUTION]
    // Icons: info-16, light-bulb-16, report-16, alert-16, stop-16
    if trimmed.starts_with("[!NOTE]") {
        Some((
            "note",
            "Note",
            r#"<svg class="octicon octicon-info mr-2" viewBox="0 0 16 16" version="1.1" width="16" height="16" aria-hidden="true"><path d="M0 8a8 8 0 1 1 16 0A8 8 0 0 1 0 8Zm8-6.5a6.5 6.5 0 1 0 0 13 6.5 6.5 0 0 0 0-13ZM6.5 7.75A.75.75 0 0 1 7.25 7h1a.75.75 0 0 1 .75.75v2.75h.25a.75.75 0 0 1 0 1.5h-2a.75.75 0 0 1 0-1.5h.25v-2h-.25a.75.75 0 0 1-.75-.75ZM8 6a1 1 0 1 1 0-2 1 1 0 0 1 0 2Z"></path></svg>"#,
        ))
    } else if trimmed.starts_with("[!TIP]") {
        Some((
            "tip",
            "Tip",
            r#"<svg class="octicon octicon-light-bulb mr-2" viewBox="0 0 16 16" version="1.1" width="16" height="16" aria-hidden="true"><path d="M8 1.5c-2.363 0-4 1.69-4 3.75 0 .984.424 1.625.984 2.304l.214.253c.223.264.47.556.673.848.284.411.537.896.621 1.49a.75.75 0 0 1-1.484.211c-.04-.282-.163-.547-.37-.847a8.456 8.456 0 0 0-.542-.68c-.084-.1-.173-.205-.268-.32C3.201 7.75 2.5 6.766 2.5 5.25 2.5 2.31 4.863 0 8 0s5.5 2.31 5.5 5.25c0 1.516-.701 2.5-1.328 3.259-.095.115-.184.22-.268.319-.207.245-.383.453-.541.681-.208.3-.33.565-.37.847a.751.751 0 0 1-1.485-.212c.084-.593.337-1.078.621-1.489.203-.292.45-.584.673-.848.075-.088.147-.173.213-.253.561-.679.985-1.32.985-2.304 0-2.06-1.637-3.75-4-3.75ZM5.75 12h4.5a.75.75 0 0 1 0 1.5h-4.5a.75.75 0 0 1 0-1.5ZM6 15.25a.75.75 0 0 1 .75-.75h2.5a.75.75 0 0 1 0 1.5h-2.5a.75.75 0 0 1-.75-.75Z"></path></svg>"#,
        ))
    } else if trimmed.starts_with("[!IMPORTANT]") {
        Some((
            "important",
            "Important",
            r#"<svg class="octicon octicon-report mr-2" viewBox="0 0 16 16" version="1.1" width="16" height="16" aria-hidden="true"><path d="M0 1.75C0 .784.784 0 1.75 0h12.5C15.216 0 16 .784 16 1.75v9.5A1.75 1.75 0 0 1 14.25 13H8.06l-2.573 2.573A1.458 1.458 0 0 1 3 14.543V13H1.75A1.75 1.75 0 0 1 0 11.25Zm1.75-.25a.25.25 0 0 0-.25.25v9.5c0 .138.112.25.25.25h2a.75.75 0 0 1 .75.75v2.19l2.72-2.72a.749.749 0 0 1 .53-.22h6.5a.25.25 0 0 0 .25-.25v-9.5a.25.25 0 0 0-.25-.25Zm7 2.25v2.5a.75.75 0 0 1-1.5 0v-2.5a.75.75 0 0 1 1.5 0ZM9 9a1 1 0 1 1-2 0 1 1 0 0 1 2 0Z"></path></svg>"#,
        ))
    } else if trimmed.starts_with("[!WARNING]") {
        Some((
            "warning",
            "Warning",
            r#"<svg class="octicon octicon-alert mr-2" viewBox="0 0 16 16" version="1.1" width="16" height="16" aria-hidden="true"><path d="M6.457 1.047c.659-1.234 2.427-1.234 3.086 0l6.082 11.378A1.75 1.75 0 0 1 14.082 15H1.918a1.75 1.75 0 0 1-1.543-2.575Zm1.763.707a.25.25 0 0 0-.44 0L1.698 13.132a.25.25 0 0 0 .22.368h12.164a.25.25 0 0 0 .22-.368Zm.53 3.996v2.5a.75.75 0 0 1-1.5 0v-2.5a.75.75 0 0 1 1.5 0ZM9 11a1 1 0 1 1-2 0 1 1 0 0 1 2 0Z"></path></svg>"#,
        ))
    } else if trimmed.starts_with("[!CAUTION]") {
        Some((
            "caution",
            "Caution",
            r#"<svg class="octicon octicon-stop mr-2" viewBox="0 0 16 16" version="1.1" width="16" height="16" aria-hidden="true"><path d="M4.47.22A.749.749 0 0 1 5 0h6c.199 0 .389.079.53.22l4.25 4.25c.141.14.22.331.22.53v6a.749.749 0 0 1-.22.53l-4.25 4.25A.749.749 0 0 1 11 16H5a.749.749 0 0 1-.53-.22L.22 11.53A.749.749 0 0 1 0 11V5c0-.199.079-.389.22-.53Zm.84 1.28L1.5 5.31v5.38l3.81 3.81h5.38l3.81-3.81V5.31L10.69 1.5ZM8 4a.75.75 0 0 1 .75.75v3.5a.75.75 0 0 1-1.5 0v-3.5A.75.75 0 0 1 8 4Zm0 8a1 1 0 1 1 0-2 1 1 0 0 1 0 2Z"></path></svg>"#,
        ))
    } else {
        None
    }
}

pub fn to_html(markdown_content: &str) -> Result<String, Error> {
    use pulldown_cmark::{CowStr, Event, Tag, TagEnd};

    let mut options = pulldown_cmark::Options::empty();
    options.insert(pulldown_cmark::Options::ENABLE_TABLES);
    options.insert(pulldown_cmark::Options::ENABLE_STRIKETHROUGH);
    options.insert(pulldown_cmark::Options::ENABLE_TASKLISTS);
    options.insert(pulldown_cmark::Options::ENABLE_HEADING_ATTRIBUTES);

    let parser = pulldown_cmark::Parser::new_ext(markdown_content, options);

    let mut html_output = String::new();
    let mut heading_text = String::new();

    let events: Vec<Event> = parser.collect();
    let mut processed_events = Vec::new();

    let mut i = 0;
    while i < events.len() {
        match &events[i] {
            Event::Start(Tag::Heading {
                level,
                id: _,
                classes,
                attrs,
            }) => {
                heading_text.clear();

                // Collect heading text
                let mut j = i + 1;
                while j < events.len() {
                    match &events[j] {
                        Event::Text(text) | Event::Code(text) => {
                            heading_text.push_str(text);
                            j += 1;
                        }
                        Event::End(TagEnd::Heading(_)) => {
                            break;
                        }
                        _ => {
                            j += 1;
                        }
                    }
                }

                // Strip backticks and generate slug for heading (same as TOC does)
                let heading_text_without_backticks = heading_text.replace('`', "");
                let slug = toc::slugify(&heading_text_without_backticks);

                // Create heading with ID
                processed_events.push(Event::Start(Tag::Heading {
                    level: *level,
                    id: Some(slug.into()),
                    classes: classes.clone(),
                    attrs: attrs.clone(),
                }));

                i += 1;
            }
            Event::End(TagEnd::Heading(_)) => {
                processed_events.push(events[i].clone());
                i += 1;
            }
            Event::Start(Tag::BlockQuote) => {
                // Check if this is a GitHub alert by looking at the first text content
                let mut j = i + 1;
                let mut first_text = String::new();

                while j < events.len() {
                    match &events[j] {
                        Event::Text(text) => {
                            first_text.push_str(text);
                            break;
                        }
                        Event::Start(_) => {
                            j += 1;
                        }
                        Event::End(TagEnd::BlockQuote) => {
                            break;
                        }
                        _ => {
                            j += 1;
                        }
                    }
                }

                if let Some((alert_type, title, svg_icon)) = detect_github_alert(&first_text) {
                    // This is a GitHub alert - transform it to custom HTML
                    // Find the end of the blockquote
                    let mut end_idx = i + 1;
                    let mut depth = 1;
                    while end_idx < events.len() && depth > 0 {
                        match &events[end_idx] {
                            Event::Start(Tag::BlockQuote) => depth += 1,
                            Event::End(TagEnd::BlockQuote) => depth -= 1,
                            _ => {}
                        }
                        end_idx += 1;
                    }

                    // Emit custom HTML for GitHub alert
                    processed_events.push(Event::Html(CowStr::from(
                        format!(r#"<div class="markdown-alert markdown-alert-{alert_type}"><p class="markdown-alert-title">{svg_icon}{title}</p>"#)
                    )));

                    // Process inner content, skipping the alert marker text
                    let mut skip_first_text = true;
                    for event in events.iter().skip(i + 1).take(end_idx - i - 1) {
                        match event {
                            Event::Text(text) if skip_first_text => {
                                // Remove the [!TYPE] marker from the text
                                let cleaned = text.trim_start();
                                if let Some(content_start) = cleaned.find(']') {
                                    let remaining = &cleaned[content_start + 1..].trim_start();
                                    if !remaining.is_empty() {
                                        processed_events
                                            .push(Event::Text(CowStr::from(remaining.to_string())));
                                    }
                                }
                                skip_first_text = false;
                            }
                            Event::End(TagEnd::BlockQuote) => {
                                // Don't emit the blockquote end
                            }
                            Event::Start(Tag::BlockQuote) => {
                                // Don't emit nested blockquote start if it's the outer one
                            }
                            _ => {
                                processed_events.push(event.clone());
                            }
                        }
                    }

                    // Close the alert div
                    processed_events.push(Event::Html(CowStr::from("</div>")));

                    i = end_idx;
                } else {
                    // Regular blockquote
                    processed_events.push(events[i].clone());
                    i += 1;
                }
            }
            _ => {
                processed_events.push(events[i].clone());
                i += 1;
            }
        }
    }

    pulldown_cmark::html::push_html(&mut html_output, processed_events.into_iter());

    Ok(html_output)
}

fn process_message(msg: Message) -> Result<serde_json::Value, Error> {
    let res = match msg {
        Message::FileChanged(path) => {
            let markdown_content = std::fs::read_to_string(&path)?;
            let html = to_html(&markdown_content)?;
            let line_count = markdown_content.lines().count();
            serde_json::json!({
              "type": "update_content",
              "data": html,
              "source_lines": line_count,
              "file_path": path,
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
    };
    Ok(res)
}

// Worker message that the websocket server deals with.
#[derive(Debug, Clone)]
pub enum Message {
    /// Markdown file was modified.
    FileChanged(String),
    /// Refresh the page with given html content.
    UpdateContent(String),
    /// Scroll to the given position specified in a percent to the window height.
    Scroll(usize),
}

/// Spawns a polling-based file watcher as a fallback when inotify fails.
/// Checks the file's modification time every second.
/// Returns a tuple of (receiver, shutdown_sender).
fn spawn_polling_file_watcher(
    file_path: String,
) -> (Receiver<Message>, std::sync::mpsc::Sender<()>) {
    let (msg_tx, msg_rx) = tokio::sync::watch::channel(Message::UpdateContent(String::new()));
    let (shutdown_tx, shutdown_rx) = std::sync::mpsc::channel();

    tokio::spawn(async move {
        let path = std::path::Path::new(&file_path);
        let mut last_mtime = std::fs::metadata(&path).and_then(|m| m.modified()).ok();

        tracing::info!(
            path = %file_path,
            "Started polling-based file watcher (checking every second)"
        );

        loop {
            // Check for shutdown signal
            if shutdown_rx.try_recv().is_ok() {
                tracing::debug!("Polling file watcher shutting down");
                break;
            }

            // Sleep for 1 second
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

            // Check if file was modified
            if let Ok(metadata) = std::fs::metadata(&path) {
                if let Ok(current_mtime) = metadata.modified() {
                    if let Some(last) = last_mtime {
                        if current_mtime > last {
                            tracing::debug!(path = %file_path, "File modified, sending update");
                            msg_tx.send_replace(Message::FileChanged(file_path.clone()));
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
/// Returns a tuple of (receiver, shutdown_sender).
fn spawn_file_watcher(
    file_path: String,
) -> Result<(Receiver<Message>, std::sync::mpsc::Sender<()>), Error> {
    let (msg_tx, msg_rx) = tokio::sync::watch::channel(Message::UpdateContent(String::new()));
    // Use a standard sync channel for the notify callback
    let (event_tx, event_rx) = std::sync::mpsc::channel();

    // Create a shutdown channel
    let (shutdown_tx, shutdown_rx) = std::sync::mpsc::channel();

    // Create a channel to signal if watcher started successfully
    let (started_tx, started_rx) = std::sync::mpsc::channel();

    // Clone file_path for different contexts
    let file_path_for_async = file_path.clone();
    let file_path_for_thread = file_path.clone();

    // Get the parent directory to watch (needed for write-rename editors)
    // Watching the file directly fails when editors remove and recreate it
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

    // Spawn the file watcher in a blocking thread since notify is not async
    let shutdown_rx_clone = shutdown_rx;
    std::thread::spawn(move || {
        let mut watcher = match RecommendedWatcher::new(
            move |res: Result<NotifyEvent, notify::Error>| {
                match res {
                    Ok(event) => {
                        // Log all events for debugging
                        tracing::debug!(?event, target_file = ?file_name_for_filter, "File watcher received event");

                        // Filter events to only our target file
                        let is_target_file = event
                            .paths
                            .iter()
                            .any(|p| p.file_name() == Some(&file_name_for_filter));

                        if !is_target_file {
                            tracing::debug!(?event.paths, "Event not for target file, ignoring");
                            return;
                        }

                        // Capture all relevant file change events:
                        // - Modify: direct file modification (Claude's Edit tool, direct edits)
                        // - Create: file created (some editors use write-rename strategy)
                        // - Remove: old file removed during write-rename (triggers reload)
                        // - Access: Some systems trigger this on write
                        // Note: We're permissive here to catch all possible write scenarios
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

        // Watch the parent directory instead of the file itself
        // This ensures we don't lose the watch when editors remove/recreate the file
        if let Err(err) = watcher.watch(&watch_target, RecursiveMode::NonRecursive) {
            tracing::error!(?err, path = ?watch_target, "Failed to watch directory");
            let _ = started_tx.send(Err(err.to_string()));
            return;
        }

        // Signal that watcher started successfully
        let _ = started_tx.send(Ok(()));

        tracing::debug!(
            watch_dir = ?watch_target,
            target_file = ?file_name,
            "File watcher started on parent directory"
        );

        // Keep the watcher alive until shutdown signal is received
        // Use recv_timeout to periodically check for shutdown
        loop {
            match shutdown_rx_clone.recv_timeout(std::time::Duration::from_secs(1)) {
                Ok(_) | Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                    tracing::debug!(path = ?file_path_for_thread, "File watcher shutting down");
                    break;
                }
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                    // Continue watching
                }
            }
        }
        // Watcher is dropped here, cleaning up file descriptors
    });

    // Spawn a blocking task to bridge sync channel to async
    tokio::task::spawn_blocking(move || {
        while let Ok(()) = event_rx.recv() {
            tracing::debug!(path = ?file_path_for_async, "File changed detected by watcher, bridging to async channel");
            // Check if there are still receivers
            let receiver_count = msg_tx.receiver_count();
            tracing::debug!(receiver_count, "Current receiver count");
            if receiver_count == 0 {
                tracing::debug!("No receivers left, exiting bridge task");
                break;
            }
            msg_tx.send_replace(Message::FileChanged(file_path_for_async.clone()));
            tracing::debug!("Message sent via send_replace, waiting for next file event");
        }
        tracing::debug!("File watcher bridge task exiting - event_rx closed");
    });

    // Wait for the watcher thread to signal whether it started successfully
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

/// Configuration for opening a markdown preview in the browser
pub struct PreviewConfig {
    /// TCP listener for the web server
    pub listener: tokio::net::TcpListener,
    /// Receiver for messages from Vim
    pub msg_rx: Receiver<Message>,
    /// Receiver for graceful shutdown signal
    pub shutdown_rx: tokio::sync::oneshot::Receiver<()>,
    /// Optional file path to watch for changes
    pub file_path: Option<String>,
    /// Optional sender to notify when browser disconnects
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

    // Create watcher channels if file_path is provided
    let (watcher_rx, _watcher_shutdown) = if let Some(path) = file_path {
        // Try inotify-based watcher first, fall back to polling if it fails
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
                // Fall back to polling-based watcher
                let (polling_rx, shutdown) = spawn_polling_file_watcher(path);
                (Some(polling_rx), Some(shutdown))
            }
        }
    } else {
        (None, None)
    };

    // Wrap disconnect_tx in Arc<Mutex<Option<>>> so it can be shared and cloned
    let disconnect_tx_shared = Arc::new(Mutex::new(disconnect_tx));

    let app = Router::new()
        .route("/", get(ws_handler))
        .layer(Extension(msg_rx))
        .layer(Extension(watcher_rx))
        .layer(Extension(disconnect_tx_shared));

    let port = listener.local_addr()?.port();

    webbrowser::open(&format!("http://127.0.0.1:{port}"))?;

    tracing::debug!("Listening on {listener:?}");

    // Use graceful shutdown so the server can be stopped externally
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(async move {
        let _ = shutdown_rx.await;
        tracing::debug!("Received shutdown signal for preview server");
    })
    .await?;

    // When this function exits, _watcher_shutdown is dropped, which sends the shutdown signal
    // to the watcher thread, allowing it to exit cleanly
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
