mod types;

use serde::{de::DeserializeOwned, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::io::{BufRead, Write};
use std::sync::atomic::{AtomicU64, Ordering};
use thiserror::Error;
use tokio::sync::mpsc::error::SendError;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};
use tokio::sync::oneshot;

pub use self::types::{
    Error, ErrorCode, Failure, Params, RpcMessage, RpcNotification, RpcRequest, RpcResponse,
    Success, VimMessage,
};

#[derive(Debug, Error)]
pub enum RpcError {
    #[error("failed to send raw message: {0}")]
    SendRawMessage(#[from] SendError<RpcMessage>),
    #[error("failed to send call: {0}")]
    SendCall(#[from] SendError<VimMessage>),
    #[error("failed to send request: {0}")]
    SendRequest(#[from] SendError<(u64, oneshot::Sender<RpcResponse>)>),
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
}

#[derive(Serialize, Debug)]
pub struct RpcClient {
    /// Id of request to Vim created from the Rust side.
    #[serde(skip_serializing)]
    id: AtomicU64,
    /// Sender for sending message from Rust to Vim.
    #[serde(skip_serializing)]
    writer_sender: UnboundedSender<RpcMessage>,
    /// Sender for passing the Vim response of request initiated from Rust.
    #[serde(skip_serializing)]
    response_sender_tx: UnboundedSender<(u64, oneshot::Sender<RpcResponse>)>,
}

impl RpcClient {
    /// Creates a new instance of [`RpcClient`].
    ///
    /// # Arguments
    ///
    /// * `reader`: a buffer reader on top of [`std::io::Stdin`].
    /// * `writer`: a buffer writer on top of [`std::io::Stdout`].
    pub fn new(
        reader: impl BufRead + Send + 'static,
        writer: impl Write + Send + 'static,
        sink: UnboundedSender<VimMessage>,
    ) -> Self {
        // Channel for passing through the response from Vim to Rust.
        let (response_sender_tx, response_sender_rx): (
            UnboundedSender<(u64, oneshot::Sender<RpcResponse>)>,
            _,
        ) = unbounded_channel();

        // A blocking task is necessary!
        tokio::task::spawn_blocking(move || {
            if let Err(error) = loop_read(reader, response_sender_rx, &sink) {
                tracing::error!(?error, "Thread stdio-reader exited");
            }
        });

        let (writer_sender, io_writer_receiver) = unbounded_channel();
        // No blocking task.
        tokio::spawn(async move {
            if let Err(error) = loop_write(writer, io_writer_receiver).await {
                tracing::error!(?error, "Thread stdio-writer exited");
            }
        });

        Self {
            id: Default::default(),
            response_sender_tx,
            writer_sender,
        }
    }

    /// Calls `call(method, params)` into Vim and return the result.
    pub async fn request<R: DeserializeOwned>(
        &self,
        method: impl AsRef<str>,
        params: impl Serialize,
    ) -> Result<R, RpcError> {
        let id = self.id.fetch_add(1, Ordering::SeqCst);
        let rpc_request = RpcRequest {
            id,
            method: method.as_ref().to_owned(),
            // call(method, args) where args expects a List in Vim, hence convert the params
            // to List unconditionally.
            params: to_array_or_none(params)?,
        };
        let (request_result_tx, request_result_rx) = oneshot::channel();
        // Request result will be sent back in a RpcResponse message.
        self.response_sender_tx.send((id, request_result_tx))?;
        self.writer_sender.send(RpcMessage::Request(rpc_request))?;
        match request_result_rx.await? {
            RpcResponse::Success(ok) => Ok(serde_json::from_value(ok.result)?),
            RpcResponse::Failure(err) => Err(RpcError::Request(format!(
                "RpcClient request error: {err:?}"
            ))),
        }
    }

    /// Sends a notification message to Vim.
    pub fn notify(&self, method: impl AsRef<str>, params: impl Serialize) -> Result<(), RpcError> {
        let notification = RpcNotification {
            method: method.as_ref().to_owned(),
            // call(method, args) where args expects a List in Vim, hence convert the params
            // to List unconditionally.
            params: to_array_or_none(params)?,
        };

        self.writer_sender
            .send(RpcMessage::Notification(notification))?;

        Ok(())
    }

    /// Sends the response of request initiated from Vim.
    pub fn send_response(
        &self,
        id: u64,
        output_result: Result<impl Serialize, RpcError>,
    ) -> Result<(), RpcError> {
        let rpc_response = match output_result {
            Ok(ok) => RpcResponse::Success(Success {
                id,
                result: serde_json::to_value(ok)?,
            }),
            Err(err) => RpcResponse::Failure(Failure {
                id,
                error: Error {
                    code: ErrorCode::InternalError,
                    message: format!("{err:?}"),
                    data: None,
                },
            }),
        };

        self.writer_sender
            .send(RpcMessage::Response(rpc_response))?;

        Ok(())
    }
}

/// Keep reading and processing the line from stdin.
fn loop_read(
    mut reader: impl BufRead,
    mut response_sender_rx: UnboundedReceiver<(u64, oneshot::Sender<RpcResponse>)>,
    sink: &UnboundedSender<VimMessage>,
) -> Result<(), RpcError> {
    let mut pending_response_senders = HashMap::new();

    loop {
        let mut line = String::new();
        match reader.read_line(&mut line) {
            Ok(number) => {
                if number > 0 {
                    match serde_json::from_str::<RpcMessage>(line.trim()) {
                        Ok(rpc_message) => match rpc_message {
                            RpcMessage::Request(rpc_request) => {
                                sink.send(VimMessage::Request(rpc_request))?;
                            }
                            RpcMessage::Notification(notification) => {
                                sink.send(VimMessage::Notification(notification))?;
                            }
                            RpcMessage::Response(response) => {
                                while let Ok((id, response_sender)) = response_sender_rx.try_recv()
                                {
                                    pending_response_senders.insert(id, response_sender);
                                }

                                if let Some(response_sender) =
                                    pending_response_senders.remove(response.id())
                                {
                                    response_sender.send(response).map_err(|response| {
                                        tracing::debug!("Failed to send response: {response:?}");
                                        RpcError::SendResponse(response)
                                    })?;
                                }
                            }
                        },
                        Err(err) => {
                            tracing::error!(error = ?err, ?line, "Invalid raw Vim message");
                        }
                    }
                } else {
                    println!("EOF reached");
                }
            }
            Err(error) => println!("Failed to read_line, error: {error}"),
        }
    }
}

/// Keep writing the response from Rust backend to Vim via stdout.
async fn loop_write(
    mut writer: impl Write,
    mut io_writer_receiver: UnboundedReceiver<RpcMessage>,
) -> Result<(), RpcError> {
    while let Some(msg) = io_writer_receiver.recv().await {
        let s = serde_json::to_string(&msg)?;

        if s.len() < 128 {
            tracing::trace!(?msg, "=> Vim");
        } else {
            let msg_size = s.len();
            match msg {
                RpcMessage::Request(request) => {
                    tracing::trace!(method = ?request.method, msg_size, "=> Vim Request")
                }
                RpcMessage::Response(response) => {
                    tracing::trace!(id = response.id(), msg_size, "=> Vim Response")
                }
                RpcMessage::Notification(notification) => {
                    tracing::trace!(
                        method = ?notification.method,
                        msg_size,
                        "=> Vim Notification"
                    )
                }
            }
        }

        // Use different convention for two reasons,
        // 1. If using '\r\ncontent', nvim will receive output as `\r` + `content`, while vim
        // receives `content`.
        // 2. Without last line ending, vim output handler won't be triggered.
        write!(writer, "Content-length: {}\n\n{}\n", s.len(), s)?;
        writer.flush()?;
    }

    Ok(())
}

fn to_array_or_none(value: impl Serialize) -> Result<Params, RpcError> {
    let json_value = serde_json::to_value(value)?;

    let params = match json_value {
        Value::Null => Params::None,
        Value::Array(vec) => Params::Array(vec),
        Value::Bool(_) | Value::Number(_) | Value::String(_) | Value::Object(_) => {
            Params::Array(vec![json_value])
        }
    };

    Ok(params)
}
