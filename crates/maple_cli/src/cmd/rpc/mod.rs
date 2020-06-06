mod filer;
mod on_move;
mod types;

use crossbeam_channel::Sender;
use log::{debug, error};
use serde::Serialize;
use serde_json::json;
use std::convert::TryFrom;
use std::io::prelude::*;
use std::thread;
use types::Message;

fn write_response<T: Serialize>(msg: T) {
    if let Ok(s) = serde_json::to_string(&msg) {
        println!("Content-length: {}\n\n{}", s.len(), s);
    }
}

fn loop_read(reader: impl BufRead, sink: &Sender<String>) {
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

fn loop_handle_message(rx: &crossbeam_channel::Receiver<String>) {
    for msg in rx.iter() {
        thread::spawn(move || {
            // Ignore the invalid message.
            if let Ok(msg) = serde_json::from_str::<Message>(&msg.trim()) {
                debug!("Recv: {:?}", msg);
                match &msg.method[..] {
                    "filer" => filer::handle_message(msg),
                    "client.on_move" => {
                        let msg_id = msg.id;
                        if let Err(e) = on_move::OnMoveHandler::try_from(msg).map(|x| x.handle()) {
                            write_response(json!({ "error": format!("{}",e), "id": msg_id }));
                        }
                    }
                    _ => write_response(
                        json!({ "error": format!("unknown method: {}", &msg.method[..]), "id": msg.id }),
                    ),
                }
            } else {
                error!("Invalid message: {:?}", msg);
            }
        });
    }
}

pub fn run_forever<R>(reader: R)
where
    R: BufRead + Send + 'static,
{
    let (tx, rx) = crossbeam_channel::unbounded();
    thread::Builder::new()
        .name("reader".into())
        .spawn(move || {
            loop_read(reader, &tx);
        })
        .expect("Failed to spawn rpc reader thread");
    loop_handle_message(&rx);
}
