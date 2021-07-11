mod previewer;
mod session;
mod types;

use std::io::prelude::*;
use std::ops::Deref;

use crossbeam_channel::{Receiver, Sender};
use log::{debug, error};
use once_cell::sync::OnceCell;
use serde::{Deserialize, Serialize};
use serde_json::json;

use self::session::{
    dumb_jump,
    filer::{self, FilerSession},
    message_handlers, quickfix, GeneralSession, SessionEvent, SessionManager,
};
use self::types::{GlobalEnv, Message};

static GLOBAL_ENV: OnceCell<GlobalEnv> = OnceCell::new();

/// Ensure GLOBAL_ENV has been instalized before using it.
pub fn global() -> impl Deref<Target = GlobalEnv> {
    if let Some(x) = GLOBAL_ENV.get() {
        x
    } else if cfg!(debug_assertions) {
        panic!("Uninitalized static: GLOBAL_ENV")
    } else {
        unreachable!("Never forget to intialize before using it!")
    }
}

fn initialize_global(msg: Message) {
    #[derive(Deserialize)]
    struct Params {
        is_nvim: Option<bool>,
        enable_icon: Option<bool>,
        clap_preview_size: serde_json::Value,
    }
    let Params {
        is_nvim,
        enable_icon,
        clap_preview_size,
    } = msg
        .deserialize_params()
        .unwrap_or_else(|e| panic!("Failed to deserialize global params: {:?}", e));

    let is_nvim = is_nvim.unwrap_or(false);
    let enable_icon = enable_icon.unwrap_or(false);

    let global_env = GlobalEnv::new(is_nvim, enable_icon, clap_preview_size.into());

    if let Err(e) = GLOBAL_ENV.set(global_env) {
        debug!("failed to initialized GLOBAL_ENV, error: {:?}", e);
    } else {
        debug!("GLOBAL_ENV initialized successfully");
    }
}

/// Writes the response to stdout.
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
    use dumb_jump::DumbJumpSession;
    use SessionEvent::*;

    let mut manager = SessionManager::default();
    for msg in rx.iter() {
        if let Ok(msg) = serde_json::from_str::<Message>(&msg.trim()) {
            debug!("==> stdio message(in): {:?}", msg);
            match &msg.method[..] {
                "initialize_global_env" => initialize_global(msg), // should be called only once.
                "init_ext_map" => message_handlers::parse_filetypedetect(msg),
                "preview/file" => message_handlers::preview_file(msg),
                "filer" => filer::handle_filer_message(msg),
                "quickfix" => quickfix::preview_quickfix_entry(msg),

                "dumb_jump/on_init" => manager.new_session::<DumbJumpSession>(msg),
                "dumb_jump/on_typed" => manager.send(msg.session_id, OnTyped(msg)),
                "dumb_jump/on_move" => manager.send(msg.session_id, OnMove(msg)),

                "filer/on_init" => manager.new_session::<FilerSession>(msg),
                "filer/on_move" => manager.send(msg.session_id, OnMove(msg)),

                "on_init" => manager.new_session::<GeneralSession>(msg),
                "on_typed" => manager.send(msg.session_id, OnTyped(msg)),
                "on_move" => manager.send(msg.session_id, OnMove(msg)),
                "exit" => manager.terminate(msg.session_id),

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
    tokio::spawn(async move {
        loop_read_rpc_message(reader, &tx);
    });

    loop_handle_rpc_message(&rx);
}
