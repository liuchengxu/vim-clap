use std::io::BufRead;

use anyhow::Result;
use crossbeam_channel::{Receiver, Sender};
use once_cell::sync::OnceCell;
use serde::Serialize;
use serde_json::json;

use crate::stdio_server::impls::dumb_jump::DumbJumpProvider;
use crate::stdio_server::impls::filer::FilerProvider;
use crate::stdio_server::impls::recent_files::RecentFilesProvider;
use crate::stdio_server::impls::DefaultProvider;
use crate::stdio_server::rpc::{Call, RpcClient};
use crate::stdio_server::session::{ProviderEvent, SessionContext, SessionManager};

/// Writes the response to stdout.
pub fn write_response<T: Serialize>(msg: T) {
    if let Ok(s) = serde_json::to_string(&msg) {
        println!("Content-length: {}\n\n{}", s.len(), s);
    }
}

fn loop_read_rpc_message(reader: impl BufRead, sink: &Sender<String>) {
    let mut reader = reader;
    loop {
        let mut message = String::new();
        match reader.read_line(&mut message) {
            Ok(number) => {
                if number > 0 {
                    if let Err(e) = sink.send(message) {
                        println!("Failed to send message, error: {}", e);
                    }
                } else {
                    println!("EOF reached");
                }
            }
            Err(error) => println!("Failed to read_line, error: {}", error),
        }
    }
}

fn loop_handle_rpc_message(rx: &Receiver<String>) {
    use ProviderEvent::*;

    let mut manager = SessionManager::default();
    for msg in rx.iter() {
        if let Ok(call) = serde_json::from_str::<Call>(msg.trim()) {
            // TODO: fix the clone
            match call.clone() {
                Call::Notification(notification) => match notification.method.as_str() {
                    "exit" => manager.terminate(notification.session_id),
                    "on_init" => {
                        let context: SessionContext = call.clone().into();
                        manager.new_session(call, Box::new(DefaultProvider::new(context)))
                    }
                    _ => {
                        tokio::spawn(async move {
                            if let Err(e) = notification.process().await {
                                tracing::error!(?e, "Error occurred when handling notification")
                            }
                        });
                    }
                },
                Call::MethodCall(method_call) => {
                    let msg = method_call;

                    if msg.method != "init_ext_map" {
                        tracing::debug!(?msg, "🔽 stdio message(in)");
                    }

                    match msg.method.as_str() {
                        "init_ext_map" => {
                            write_response(msg.parse_filetypedetect());
                        }
                        "preview/file" => {
                            tokio::spawn(async move {
                                match msg.preview_file().await {
                                    Ok(res) => write_response(res),
                                    Err(e) => tracing::error!(?e, "Failed to preview file"),
                                }
                            });
                        }
                        "quickfix" => {
                            tokio::spawn(async move {
                                match msg.preview_quickfix().await {
                                    Ok(res) => write_response(res),
                                    Err(e) => tracing::error!(?e, "Failed to preview quickfix"),
                                }
                            });
                        }

                        "dumb_jump/on_init" => {
                            let context: SessionContext = call.clone().into();
                            manager.new_session(call, Box::new(DumbJumpProvider::new(context)))
                        }
                        "dumb_jump/on_typed" => manager.send(msg.session_id, OnTyped(msg)),
                        "dumb_jump/on_move" => manager.send(msg.session_id, OnMove(msg)),

                        "recent_files/on_init" => {
                            let context: SessionContext = call.clone().into();
                            manager.new_session(call, Box::new(RecentFilesProvider::new(context)))
                        }
                        "recent_files/on_typed" => manager.send(msg.session_id, OnTyped(msg)),
                        "recent_files/on_move" => manager.send(msg.session_id, OnMove(msg)),

                        "filer/on_init" => {
                            let context: SessionContext = call.clone().into();
                            manager.new_session(call, Box::new(FilerProvider::new(context)))
                        }
                        "filer/on_typed" => manager.send(msg.session_id, OnTyped(msg)),
                        "filer/on_move" => manager.send(msg.session_id, OnMove(msg)),

                        "on_typed" => manager.send(msg.session_id, OnTyped(msg)),
                        "on_move" => manager.send(msg.session_id, OnMove(msg)),

                        method => write_response(
                            json!({ "error": format!("unknown method: {}", method), "id": msg.id }),
                        ),
                    }
                }
            }
        } else {
            tracing::error!(?msg, "Invalid message");
        }
    }
}

pub fn run_forever(reader: impl BufRead + Send + 'static) {
    let (tx, rx) = crossbeam_channel::unbounded();
    tokio::spawn(async move {
        loop_read_rpc_message(reader, &tx);
    });

    loop_handle_rpc_message(&rx);
}
