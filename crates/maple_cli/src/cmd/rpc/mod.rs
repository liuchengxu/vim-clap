mod filer;
mod on_move;
mod types;

use crossbeam_channel::Sender;
use log::{debug, error};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::io::prelude::*;
use std::thread;

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Message {
    pub method: String,
    pub params: serde_json::Map<String, Value>,
    pub id: u64,
}

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
                        if let Err(e) = on_move::handle_message_on_move(msg) {
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
