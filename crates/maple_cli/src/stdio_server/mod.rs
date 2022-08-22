mod impls;
mod rpc;
mod session;
mod session_client;
mod state;
mod types;
mod vim;

use std::io::{BufReader, BufWriter};
use std::ops::Deref;
use std::sync::Arc;

use once_cell::sync::OnceCell;

use self::rpc::RpcClient;
use self::session_client::SessionClient;
use self::state::State;
use self::types::GlobalEnv;

pub use self::rpc::{MethodCall, Notification};

static GLOBAL_ENV: OnceCell<GlobalEnv> = OnceCell::new();

/// Writes the response to stdout.
pub fn write_response<T: serde::Serialize>(msg: T) {
    if let Ok(s) = serde_json::to_string(&msg) {
        println!("Content-length: {}\n\n{}", s.len(), s);
    }
}

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

/// Starts and keep running the server on top of stdio.
pub async fn start() {
    let (call_tx, call_rx) = tokio::sync::mpsc::unbounded_channel();

    let rpc_client = Arc::new(RpcClient::new(
        BufReader::new(std::io::stdin()),
        BufWriter::new(std::io::stdout()),
        call_tx.clone(),
    ));

    let state = State::new(call_tx, rpc_client);
    let session_client = SessionClient::new(state);
    session_client.loop_call(call_rx).await;
}
