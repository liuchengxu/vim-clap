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

fn process_message(msg: Message) -> Result<serde_json::Value, Error> {
    let res = match msg {
        Message::FileChanged(path) => {
            let markdown_content = std::fs::read_to_string(path)?;
            let html = markdown::to_html_with_options(
                &markdown_content,
                &markdown::Options {
                    parse: markdown::ParseOptions::gfm(),
                    compile: markdown::CompileOptions {
                        gfm_task_list_item_checkable: true,
                        ..markdown::CompileOptions::gfm()
                    },
                },
            )?;
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

// Message type between the server and ws clients.
#[derive(Debug, Clone)]
pub enum Message {
    /// Markdown file was modified.
    FileChanged(String),
    /// Refresh the page with given html content.
    UpdateContent(String),
    /// Scroll to the given position specified in a percent to the window height.
    Scroll(usize),
}

pub async fn open_preview(
    listener: tokio::net::TcpListener,
    msg_rx: Receiver<Message>,
) -> Result<(), Error> {
    let app = Router::new()
        .route("/", get(ws_handler))
        .layer(Extension(msg_rx));

    let port = listener.local_addr()?.port();

    if let Err(err) = webbrowser::open(&format!("http://127.0.0.1:{port}")) {
        tracing::error!("Error serving connection: {:?}", err);
    }

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

        open_preview(listener, msg_rx)
            .await
            .expect("Failed to open markdown preview");
    }
}
