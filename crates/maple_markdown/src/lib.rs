pub mod toc;

use axum::extract::ws::{Message as WsMessage, WebSocket, WebSocketUpgrade};
use axum::extract::Extension;
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse};
use axum::routing::get;
use axum::Router;
use std::net::SocketAddr;
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
) -> impl IntoResponse {
    if let Some(ws) = ws {
        ws.on_upgrade(|ws| async move { handle_websocket(ws, msg_rx).await })
    } else {
        let html = include_str!("../js/index.html");
        (StatusCode::OK, Html(html)).into_response()
    }
}

async fn handle_websocket(mut socket: WebSocket, mut msg_rx: Receiver<Message>) {
    while msg_rx.changed().await.is_ok() {
        let msg = msg_rx.borrow().clone();

        let Ok(text) = process_message(msg) else {
            break;
        };

        if socket
            .send(WsMessage::Text(text.to_string()))
            .await
            .is_err()
        {
            break;
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
            let markdown_content = std::fs::read_to_string(path)?;
            let html = to_html(&markdown_content)?;
            serde_json::json!({
              "type": "update_content",
              "data": html,
            })
        }
        Message::UpdateContent(content) => {
            serde_json::json!({
              "type": "update_content",
              "data": content,
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

pub async fn open_preview_in_browser(
    listener: tokio::net::TcpListener,
    msg_rx: Receiver<Message>,
) -> Result<(), Error> {
    let app = Router::new()
        .route("/", get(ws_handler))
        .layer(Extension(msg_rx));

    let port = listener.local_addr()?.port();

    webbrowser::open(&format!("http://127.0.0.1:{port}"))?;

    tracing::debug!("Listening on {listener:?}");

    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await?;

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

        open_preview_in_browser(listener, msg_rx)
            .await
            .expect("Failed to open markdown preview");
    }
}
