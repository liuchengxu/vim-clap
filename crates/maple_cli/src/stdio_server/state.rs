use std::sync::Arc;

use serde::Serialize;
use tokio::sync::mpsc::UnboundedSender;

use crate::stdio_server::rpc::{Call, RpcClient};
use crate::stdio_server::vim::Vim;

/// Current State of Vim/NeoVim client.
#[derive(Serialize)]
pub struct State {
    #[serde(skip_serializing)]
    pub tx: UnboundedSender<Call>,

    #[serde(skip_serializing)]
    pub vim: Vim,

    /// Highlight match ids.
    pub highlights: Vec<u32>,
}

impl State {
    pub fn new(tx: UnboundedSender<Call>, client: Arc<RpcClient>) -> Self {
        Self {
            tx,
            vim: Vim::new(client),
            highlights: Default::default(),
        }
    }
}
