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

pub fn to_html(markdown_content: &str) -> Result<String, Error> {
    use pulldown_cmark::{Event, Tag, TagEnd};

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
