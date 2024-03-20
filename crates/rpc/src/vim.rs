use crate::{
    Error, ErrorCode, Failure, Id, Params, RpcError, RpcMessage, RpcNotification, RpcRequest,
    RpcResponse, Success,
};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::io::{BufRead, Write};
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};
use tokio::sync::oneshot;

/// RPC message originated from Vim.
///
/// Message sent via `clap#client#notify` or `clap#client#request_async`.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(untagged)]
pub enum VimMessage {
    Request(RpcRequest),
    Notification(RpcNotification),
}

/// RPC Client talking to Vim.
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
    response_sender_tx: UnboundedSender<(Id, oneshot::Sender<RpcResponse>)>,
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
            UnboundedSender<(Id, oneshot::Sender<RpcResponse>)>,
            _,
        ) = unbounded_channel();

        std::thread::Builder::new()
            .name("stdio-reader".to_string())
            .spawn(move || {
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

    pub fn next_request_id(&self) -> u64 {
        self.id.fetch_add(1, Ordering::SeqCst)
    }

    /// Calls `call(method, params)` into Vim and return the result.
    pub async fn request<R: DeserializeOwned>(
        &self,
        method: impl AsRef<str>,
        params: impl Serialize,
    ) -> Result<R, RpcError> {
        let id = self.next_request_id();
        let rpc_request = RpcRequest {
            jsonrpc: None,
            id: Id::Num(id),
            method: method.as_ref().to_owned(),
            // call(method, args) where args expects a List in Vim, hence convert the params
            // to List unconditionally.
            params: to_array_or_none(params)?,
        };
        let (request_result_tx, request_result_rx) = oneshot::channel();
        // Request result will be sent back in a RpcResponse message.
        self.response_sender_tx
            .send((Id::Num(id), request_result_tx))?;
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
            jsonrpc: None,
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
        id: Id,
        output_result: Result<impl Serialize, RpcError>,
    ) -> Result<(), RpcError> {
        let rpc_response = match output_result {
            Ok(ok) => RpcResponse::Success(Success {
                jsonrpc: None,
                id,
                result: serde_json::to_value(ok)?,
            }),
            Err(err) => RpcResponse::Failure(Failure {
                jsonrpc: None,
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
    mut response_sender_rx: UnboundedReceiver<(Id, oneshot::Sender<RpcResponse>)>,
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
                    tracing::trace!(id = %response.id(), msg_size, "=> Vim Response")
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
