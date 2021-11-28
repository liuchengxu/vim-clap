mod deprecated_runner;
mod providers;
mod rpc;
mod session;
mod session_client;
mod state;
mod types;
mod vim;

use std::io::prelude::*;
use std::io::{BufReader, BufWriter};
use std::ops::Deref;
use std::sync::Arc;

use anyhow::Result;
use crossbeam_channel::{Receiver, Sender};
use once_cell::sync::OnceCell;
use serde::Serialize;
use serde_json::json;

use self::providers::{
    dumb_jump,
    filer::{self, FilerSession},
    quickfix, recent_files, BuiltinSession,
};
use self::rpc::{Call, RpcClient};
use self::session::{SessionEvent, SessionManager};
use self::session_client::SessionClient;
use self::state::State;
use self::types::GlobalEnv;

pub use self::deprecated_runner::{run_forever, write_response};
pub use self::rpc::{MethodCall, Notification};

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

/// Starts and keep running the server on top of stdio.
pub fn start() -> Result<()> {
    let (call_tx, call_rx) = crossbeam_channel::unbounded();

    let rpc_client = Arc::new(RpcClient::new(
        BufReader::new(std::io::stdin()),
        BufWriter::new(std::io::stdout()),
        call_tx.clone(),
    ));

    let state = State::new(call_tx, rpc_client);
    let session_client = SessionClient::new(state);
    session_client.loop_call(&call_rx);

    Ok(())
}
