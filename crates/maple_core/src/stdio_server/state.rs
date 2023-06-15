use crate::stdio_server::vim::Vim;
use rpc::{RpcClient, VimRpcMessage};
use serde::Serialize;
use std::sync::Arc;
use tokio::sync::mpsc::UnboundedSender;

/// Current State of Vim/NeoVim client.
#[derive(Serialize)]
pub struct State {
    #[serde(skip_serializing)]
    pub tx: UnboundedSender<VimRpcMessage>,

    #[serde(skip_serializing)]
    pub vim: Vim,

    /// Highlight match ids.
    pub highlights: Vec<u32>,
}

impl State {
    pub fn new(tx: UnboundedSender<VimRpcMessage>, client: Arc<RpcClient>) -> Self {
        Self {
            tx,
            vim: Vim::new(client),
            highlights: Default::default(),
        }
    }
}
