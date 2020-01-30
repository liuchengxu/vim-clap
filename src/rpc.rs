use std::io::prelude::*;
use std::{fs, io, thread};

use anyhow::Result;
use crossbeam_channel::Sender;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::icon::prepend_filer_icon;

const REQUEST_FILER: &str = "filer";

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Message {
    pub method: String,
    pub params: serde_json::Map<String, Value>,
    pub id: u64,
}

fn loop_read(reader: impl BufRead, sink: &Sender<String>) {
    let mut reader = reader;
    loop {
        let mut message = String::new();
        match reader.read_line(&mut message) {
            Ok(number) => {
                if number > 0 {
                    if let Err(e) = sink.send(message) {
                        println!("Failed to send message, error: {}", e);
                    }
                } else {
                    println!("EOF reached");
                }
            }
            Err(error) => println!("Failed to read_line, error: {}", error),
        }
    }
}

fn write_response<T: Serialize>(msg: T) {
    if let Ok(s) = serde_json::to_string(&msg) {
        println!("Content-length: {}\n\n{}", s.len(), s);
    }
}

fn handle_filer(msg: Message) {
    if let Some(dir) = msg.params.get("cwd").and_then(|x| x.as_str()) {
        let enable_icon = msg
            .params
            .get("enable_icon")
            .and_then(|x| x.as_bool())
            .unwrap_or(false);
        let result = match read_dir_entries(&dir, enable_icon) {
            Ok(entries) => {
                let result = json!({
                "entries": entries,
                "dir": dir,
                "total": entries.len(),
                });
                json!({ "result": result, "id": msg.id })
            }
            Err(err) => {
                let error = json!({"message": format!("{}", err), "dir": dir});
                json!({ "error": error, "id": msg.id })
            }
        };
        write_response(result);
    }
}

fn loop_handle_message(rx: &crossbeam_channel::Receiver<String>) {
    for msg in rx.iter() {
        thread::spawn(move || {
            // Ignore the invalid message.
            if let Ok(msg) = serde_json::from_str::<Message>(&msg.trim()) {
                match &msg.method[..] {
                    REQUEST_FILER => handle_filer(msg),
                    _ => write_response(json!({ "error": "unknown method", "id": msg.id })),
                }
            }
        });
    }
}

pub fn run_forever<R>(reader: R)
where
    R: BufRead + Send + 'static,
{
    let (tx, rx) = crossbeam_channel::unbounded();
    thread::Builder::new()
        .name("reader".into())
        .spawn(move || {
            loop_read(reader, &tx);
        })
        .expect("Failed to spawn rpc reader thread");
    loop_handle_message(&rx);
}

fn into_string(entry: std::fs::DirEntry, enable_icon: bool) -> String {
    let path_str = if entry.path().is_dir() {
        format!(
            "{}/",
            entry
                .path()
                .file_name()
                .and_then(std::ffi::OsStr::to_str)
                .unwrap()
        )
    } else {
        entry
            .path()
            .file_name()
            .and_then(std::ffi::OsStr::to_str)
            .map(Into::into)
            .unwrap()
    };

    if enable_icon {
        prepend_filer_icon(&entry.path(), &path_str)
    } else {
        path_str
    }
}

fn read_dir_entries(dir: &str, enable_icon: bool) -> Result<Vec<String>> {
    let mut entries = fs::read_dir(dir)?
        .map(|res| res.map(|x| into_string(x, enable_icon)))
        .collect::<Result<Vec<_>, io::Error>>()?;

    entries.sort();

    Ok(entries)
}

#[test]
fn test_dir() {
    let entries = read_dir_entries("/.DocumentRevisions-V100/", true).unwrap();
    println!("entry: {:?}", entries);
}
