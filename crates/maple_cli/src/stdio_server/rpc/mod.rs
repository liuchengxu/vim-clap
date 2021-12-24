mod messages;
mod types;

use std::collections::HashMap;
use std::io::{BufRead, Write};
use std::sync::atomic::{AtomicU64, Ordering};

use anyhow::{anyhow, Result};
use crossbeam_channel::{bounded, unbounded, Receiver, Sender};
use jsonrpc_core::Params;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::Value;

pub use self::messages::method_call::MethodCall;
pub use self::messages::notification::Notification;
pub use self::types::{Call, Error, Failure, Output, RawMessage, Success};

#[derive(Serialize)]
pub struct RpcClient {
    /// Id of request to Vim created from the Rust side.
    #[serde(skip_serializing)]
    id: AtomicU64,
    /// Sender for sending message from Rust to Vim.
    #[serde(skip_serializing)]
    output_writer_tx: Sender<RawMessage>,
    /// Sender for passing the Vim response of request initiated from Rust.
    #[serde(skip_serializing)]
    output_reader_tx: Sender<(u64, Sender<Output>)>,
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
        sink: Sender<Call>,
    ) -> Self {
        // Channel for passing through the response from Vim.
        let (output_reader_tx, output_reader_rx): (Sender<(u64, Sender<Output>)>, _) = unbounded();
        tokio::spawn(async move {
            if let Err(error) = loop_read(reader, output_reader_rx, &sink) {
                tracing::error!(?error, "Thread stdio-reader exited");
            }
        });

        let (output_writer_tx, output_writer_rx) = unbounded();
        tokio::spawn(async move {
            if let Err(error) = loop_write(writer, &output_writer_rx) {
                tracing::error!(?error, "Thread stdio-writer exited");
            }
        });

        Self {
            id: Default::default(),
            output_reader_tx,
            output_writer_tx,
        }
    }

    /// Calls into Vim.
    ///
    /// Wait for the Vim response until the timeout.
    pub fn call<R: DeserializeOwned>(
        &self,
        method: impl AsRef<str>,
        params: impl Serialize,
    ) -> Result<R> {
        let id = self.id.fetch_add(1, Ordering::SeqCst);
        let method_call = MethodCall {
            id,
            method: method.as_ref().to_owned(),
            params: to_params(params)?,
            session_id: 888u64, // FIXME
        };
        let (tx, rx) = bounded(1);
        self.output_reader_tx.send((id, tx))?;
        self.output_writer_tx
            .send(RawMessage::MethodCall(method_call))?;
        match rx.recv_timeout(std::time::Duration::from_secs(60))? {
            Output::Success(ok) => Ok(serde_json::from_value(ok.result)?),
            Output::Failure(err) => Err(anyhow!("Error: {:?}", err)),
        }
    }

    /// Sends a notification message to Vim.
    pub fn notify(&self, method: impl AsRef<str>, params: impl Serialize) -> Result<()> {
        let notification = Notification {
            method: method.as_ref().to_owned(),
            params: to_params(params)?,
            session_id: 888u64, // FIXME
        };

        self.output_writer_tx
            .send(RawMessage::Notification(notification))?;

        Ok(())
    }

    /// Sends the response from Rust to Vim.
    pub fn output(&self, id: u64, output_result: Result<impl Serialize>) -> Result<()> {
        let output = match output_result {
            Ok(ok) => Output::Success(Success {
                id,
                result: serde_json::to_value(ok)?,
            }),
            Err(err) => Output::Failure(Failure {
                id,
                error: Error {
                    code: jsonrpc_core::ErrorCode::InternalError,
                    message: err.to_string(),
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
    output_reader_rx: Receiver<(u64, Sender<Output>)>,
    sink: &Sender<Call>,
) -> Result<()> {
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
                                        anyhow!("Failed to send output: {:?}", output)
                                    })?;
                                }
                            }
                        },
                        Err(e) => {
                            tracing::error!(?line, "Invalid raw message");
                        }
                    }
                } else {
                    println!("EOF reached");
                }
            }
            Err(error) => println!("Failed to read_line, error: {}", error),
        }
    }
}

/// Keep writing the response from Rust backend to Vim via stdout.
fn loop_write(writer: impl Write, rx: &Receiver<RawMessage>) -> Result<()> {
    let mut writer = writer;

    for msg in rx.iter() {
        tracing::debug!(?msg, "Sending back to the Vim side");
        let s = serde_json::to_string(&msg)?;
        // Use different convention for two reasons,
        // 1. If using '\r\ncontent', nvim will receive output as `\r` + `content`, while vim
        // receives `content`.
        // 2. Without last line ending, vim output handler won't be triggered.
        write!(writer, "Content-length: {}\n\n{}\n", s.len(), s)?;
        writer.flush()?;
    }

    Ok(())
}

fn to_params(value: impl Serialize) -> Result<Params> {
    let json_value = serde_json::to_value(value)?;

    let params = match json_value {
        Value::Null => Params::None,
        Value::Bool(_) | Value::Number(_) | Value::String(_) => Params::Array(vec![json_value]),
        Value::Array(vec) => Params::Array(vec),
        Value::Object(map) => Params::Map(map),
    };

    Ok(params)
}
