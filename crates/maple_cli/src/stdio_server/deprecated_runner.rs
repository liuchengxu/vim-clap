use super::*;

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
    use dumb_jump::DumbJumpSession;
    use recent_files::RecentFilesSession;
    use SessionEvent::*;

    let mut manager = SessionManager::default();
    for msg in rx.iter() {
        if let Ok(call) = serde_json::from_str::<Call>(msg.trim()) {
            // TODO: fix the clone
            match call.clone() {
                Call::Notification(notification) => match notification.method.as_str() {
                    "exit" => manager.terminate(notification.session_id),
                    "on_init" => manager.new_session::<BuiltinSession>(call),
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
                        tracing::debug!(?msg, "==> stdio message(in)");
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

                        "dumb_jump/on_init" => manager.new_session::<DumbJumpSession>(call),
                        "dumb_jump/on_typed" => manager.send(msg.session_id, OnTyped(msg)),
                        "dumb_jump/on_move" => manager.send(msg.session_id, OnMove(msg)),

                        "recent_files/on_init" => manager.new_session::<RecentFilesSession>(call),
                        "recent_files/on_typed" => manager.send(msg.session_id, OnTyped(msg)),
                        "recent_files/on_move" => manager.send(msg.session_id, OnMove(msg)),

                        "filer" => {
                            tokio::spawn(async move {
                                write_response(
                                    filer::handle_filer_message(msg)
                                        .expect("Both Success and Error are returned"),
                                );
                            });
                        }
                        "filer/on_init" => manager.new_session::<FilerSession>(call),
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
