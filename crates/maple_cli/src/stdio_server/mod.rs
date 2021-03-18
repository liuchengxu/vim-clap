mod env;
mod session;
mod types;

use std::io::prelude::*;

use crossbeam_channel::{Receiver, Sender};
use log::{debug, error};
use serde::Serialize;
use serde_json::json;

use session::{
    dumb_jump,
    filer::{self, FilerSession},
    GeneralSession, Manager, SessionEvent,
};
use types::Message;

fn write_response<T: Serialize>(msg: T) {
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
    use SessionEvent::*;

    let mut session_manager = Manager::default();
    for msg in rx.iter() {
        if let Ok(msg) = serde_json::from_str::<Message>(&msg.trim()) {
            debug!("==> message(in): {:?}", msg);
            match &msg.method[..] {
                "initialize_global_env" => env::initialize_global(msg), // should be called only once.
                "filer" => filer::handle_filer_message(msg),
                "dumb_jump" => dumb_jump::handle_dumb_jump_message(msg),
                "filer/on_init" => session_manager.new_session(msg.session_id, msg, FilerSession),
                "filer/on_move" => session_manager.send(msg.session_id, OnMove(msg)),
                "on_init" => session_manager.new_session(msg.session_id, msg, GeneralSession),
                "on_typed" => session_manager.send(msg.session_id, OnTyped(msg)),
                "on_move" => session_manager.send(msg.session_id, OnMove(msg)),
                "exit" => session_manager.terminate(msg.session_id),
                _ => write_response(
                    json!({ "error": format!("unknown method: {}", &msg.method[..]), "id": msg.id }),
                ),
            }
        } else {
            error!("Invalid message: {:?}", msg);
        }
    }
}

pub fn run_forever<R>(reader: R)
where
    R: BufRead + Send + 'static,
{
    let (tx, rx) = crossbeam_channel::unbounded();
    let runtime = tokio::runtime::Runtime::new().unwrap();
    runtime.block_on(async {
        tokio::spawn(async move {
            loop_read_rpc_message(reader, &tx);
        });

        loop_handle_rpc_message(&rx);
    });
}
