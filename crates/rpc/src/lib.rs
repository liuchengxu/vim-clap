mod jsonrpc;
pub mod vim;

use thiserror::Error;
use tokio::sync::mpsc::error::SendError;
use tokio::sync::oneshot;

pub use self::jsonrpc::{
    Error, ErrorCode, Failure, Id, Params, RpcMessage, RpcNotification, RpcRequest, RpcResponse,
    Success, Version,
};

#[derive(Debug, Error)]
pub enum RpcError {
    #[error("failed to send raw message: {0}")]
    SendRawMessage(#[from] SendError<RpcMessage>),
    #[error("failed to send call: {0}")]
    SendCall(#[from] SendError<vim::VimMessage>),
    #[error("failed to send request: {0}")]
    SendRequest(#[from] SendError<(Id, oneshot::Sender<RpcResponse>)>),
    #[error("failed to send response: {0:?}")]
    SendResponse(RpcResponse),
    #[error("sender is dropped: {0}")]
    OneshotRecv(#[from] tokio::sync::oneshot::error::RecvError),
    #[error(transparent)]
    SerdeJson(#[from] serde_json::Error),
    #[error(transparent)]
    IO(#[from] std::io::Error),
    #[error("request failure: {0}")]
    Request(String),
    #[error("invalid server message: {0}")]
    ServerMessage(String),
    #[error("stream closed")]
    StreamClosed,
    #[error("{0}")]
    DeserializeFailure(String),
    #[error(transparent)]
    JsonRpc(#[from] Error),
}
