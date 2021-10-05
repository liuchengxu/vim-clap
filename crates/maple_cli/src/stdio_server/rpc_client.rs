use std::collections::HashMap;
use std::io::{BufRead, Write};
use std::sync::atomic::AtomicU64;

use anyhow::{anyhow, Result};
use crossbeam_channel::{unbounded, Receiver, Sender};
use log::error;
use serde::Serialize;

use crate::stdio_server::types::{Call, Output, RawMessage};

#[derive(Serialize)]
pub struct RpcClient {
    #[serde(skip_serializing)]
    id: AtomicU64,
    #[serde(skip_serializing)]
    output_writer_tx: Sender<RawMessage>,
    #[serde(skip_serializing)]
    output_reader_tx: Sender<(u64, Sender<Output>)>,
}

impl RpcClient {
    /// * reader: stdin
    /// * writer: stdout
    pub fn new(
        reader: impl BufRead + Send + 'static,
        writer: impl Write + Send + 'static,
        sink: Sender<Call>,
    ) -> Result<Self> {
        // Channel for passing through the response from Vim.
        let (output_reader_tx, output_reader_rx): (Sender<(u64, Sender<Output>)>, _) = unbounded();

        std::thread::Builder::new()
            .name("stdio-reader".into())
            .spawn(move || {
                if let Err(err) = loop_read(reader, output_reader_rx, &sink) {
                    error!("Thread stdio-reader exited with error: {:?}", err);
                }
            })?;

        let (output_writer_tx, output_writer_rx) = unbounded();

        std::thread::Builder::new()
            .name("stdio-writer".into())
            .spawn(move || {
                if let Err(err) = loop_write(writer, &output_writer_rx) {
                    error!("Thread stdio-writer exited with error: {:?}", err);
                }
            })?;

        Ok(Self {
            id: AtomicU64::default(),
            output_reader_tx,
            output_writer_tx,
        })
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
                    match serde_json::from_str::<RawMessage>(&line.trim()) {
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
                            error!("Invalid raw message: {:?}", line);
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
        let s = serde_json::to_string(&msg)?;
        // Use different convention for two reasons,
        // 1. If using '\r\ncontent', nvim will receive output as `\r` + `content`, while vim
        // receives `content`.
        // 2. Without last line ending, vim output handler won't be triggered.
        write!(writer, "Content-Length: {}\n\n{}\n", s.len(), s)?;
        writer.flush()?;
    }
    Ok(())
}
