mod types;

use serde::{de::DeserializeOwned, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::io::{BufRead, Write};
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::mpsc::error::SendError;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};
use tokio::sync::oneshot;

pub use self::types::{
    Call, Error, ErrorCode, Failure, MethodCall, Notification, Output, Params, RawMessage, Success,
};

#[derive(Debug)]
pub enum RpcError {
    SendRawMessage(SendError<RawMessage>),
    SendCall(SendError<Call>),
    SendRequest(SendError<(u64, oneshot::Sender<Output>)>),
    SendOutput(Output),
    OneshotRecv(tokio::sync::oneshot::error::RecvError),
    SerdeJson(serde_json::Error),
    IO(std::io::Error),
    Request(String),
}

impl From<SendError<RawMessage>> for RpcError {
    fn from(e: SendError<RawMessage>) -> Self {
        Self::SendRawMessage(e)
    }
}

impl From<SendError<Call>> for RpcError {
    fn from(e: SendError<Call>) -> Self {
        Self::SendCall(e)
    }
}

impl From<SendError<(u64, oneshot::Sender<Output>)>> for RpcError {
    fn from(e: SendError<(u64, oneshot::Sender<Output>)>) -> Self {
        Self::SendRequest(e)
    }
}

impl From<Output> for RpcError {
    fn from(e: Output) -> Self {
        Self::SendOutput(e)
    }
}

impl From<tokio::sync::oneshot::error::RecvError> for RpcError {
    fn from(e: tokio::sync::oneshot::error::RecvError) -> Self {
        Self::OneshotRecv(e)
    }
}

impl From<serde_json::Error> for RpcError {
    fn from(e: serde_json::Error) -> Self {
        Self::SerdeJson(e)
    }
}

impl From<std::io::Error> for RpcError {
    fn from(e: std::io::Error) -> Self {
        Self::IO(e)
    }
}

#[derive(Serialize, Debug)]
pub struct RpcClient {
    /// Id of request to Vim created from the Rust side.
    #[serde(skip_serializing)]
    id: AtomicU64,
    /// Sender for sending message from Rust to Vim.
    #[serde(skip_serializing)]
    output_writer_tx: UnboundedSender<RawMessage>,
    /// Sender for passing the Vim response of request initiated from Rust.
    #[serde(skip_serializing)]
    output_reader_tx: UnboundedSender<(u64, oneshot::Sender<Output>)>,
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
        sink: UnboundedSender<Call>,
    ) -> Self {
        // Channel for passing through the response from Vim and the request to Vim.
        let (output_reader_tx, output_reader_rx): (
            UnboundedSender<(u64, oneshot::Sender<Output>)>,
            _,
        ) = unbounded_channel();

        // A blocking task is necessary!
        tokio::task::spawn_blocking(move || {
            if let Err(error) = loop_read(reader, output_reader_rx, &sink) {
                tracing::error!(?error, "Thread stdio-reader exited");
            }
        });

        let (output_writer_tx, output_writer_rx) = unbounded_channel();
        // No blocking task.
        tokio::spawn(async move {
            if let Err(error) = loop_write(writer, output_writer_rx).await {
                tracing::error!(?error, "Thread stdio-writer exited");
            }
        });

        Self {
            id: Default::default(),
            output_reader_tx,
            output_writer_tx,
        }
    }

    /// Calls `call(method, params)` into Vim and return the result.
    pub async fn request<R: DeserializeOwned>(
        &self,
        method: impl AsRef<str>,
        params: impl Serialize,
    ) -> Result<R, RpcError> {
        let id = self.id.fetch_add(1, Ordering::SeqCst);
        let method_call = MethodCall {
            id,
            method: method.as_ref().to_owned(),
            // call(method, args) where args expects a List in Vim, hence convert the params
            // to List unconditionally.
            params: to_array_or_none(params)?,
            session_id: 0u64, // Unused for now.
        };
        let (tx, rx) = oneshot::channel();
        self.output_reader_tx.send((id, tx))?;
        self.output_writer_tx
            .send(RawMessage::MethodCall(method_call))?;
        match rx.await? {
            Output::Success(ok) => Ok(serde_json::from_value(ok.result)?),
            Output::Failure(err) => Err(RpcError::Request(format!(
                "RpcClient request error: {err:?}"
            ))),
        }
    }

    /// Sends a notification message to Vim.
    pub fn notify(&self, method: impl AsRef<str>, params: impl Serialize) -> Result<(), RpcError> {
        let notification = Notification {
            method: method.as_ref().to_owned(),
            // call(method, args) where args expects a List in Vim, hence convert the params
            // to List unconditionally.
            params: to_array_or_none(params)?,
            session_id: None,
        };

        self.output_writer_tx
            .send(RawMessage::Notification(notification))?;

        Ok(())
    }

    /// Sends the response from Rust to Vim.
    pub fn output(
        &self,
        id: u64,
        output_result: Result<impl Serialize, RpcError>,
    ) -> Result<(), RpcError> {
        let output = match output_result {
            Ok(ok) => Output::Success(Success {
                id,
                result: serde_json::to_value(ok)?,
            }),
            Err(err) => Output::Failure(Failure {
                id,
                error: Error {
                    code: ErrorCode::InternalError,
                    message: format!("{err:?}"),
                    data: None,
                },
            }),
        };

        self.output_writer_tx.send(RawMessage::Output(output))?;

        Ok(())
    }
}

/// Keep reading and processing the line from stdin.
fn loop_read(
    reader: impl BufRead,
    mut output_reader_rx: UnboundedReceiver<(u64, oneshot::Sender<Output>)>,
    sink: &UnboundedSender<Call>,
) -> Result<(), RpcError> {
    let mut pending_outputs = HashMap::new();

    let mut reader = reader;
    loop {
        let mut line = String::new();
        match reader.read_line(&mut line) {
            Ok(number) => {
                if number > 0 {
                    match serde_json::from_str::<RawMessage>(line.trim()) {
                        Ok(raw_message) => match raw_message {
                            RawMessage::MethodCall(method_call) => {
                                sink.send(Call::MethodCall(method_call))?;
                            }
                            RawMessage::Notification(notification) => {
                                sink.send(Call::Notification(notification))?;
                            }
                            RawMessage::Output(output) => {
                                while let Ok((id, tx)) = output_reader_rx.try_recv() {
                                    pending_outputs.insert(id, tx);
                                }

                                if let Some(tx) = pending_outputs.remove(output.id()) {
                                    tx.send(output).map_err(|output| {
                                        tracing::debug!("Failed to send output: {output:?}");
                                        RpcError::SendOutput(output)
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
    writer: impl Write,
    mut rx: UnboundedReceiver<RawMessage>,
) -> Result<(), RpcError> {
    let mut writer = writer;

    while let Some(msg) = rx.recv().await {
        let s = serde_json::to_string(&msg)?;
        if s.len() < 100 {
            tracing::trace!(?msg, "=> Vim");
        } else {
            tracing::trace!(msg_size = ?s.len(), msg_kind = msg.kind(), method = ?msg.method(), "=> Vim");
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
