use std::io::prelude::*;
use std::{fs, io, thread};

use anyhow::Result;
use crossbeam_channel::Sender;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

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

fn handle_filer(msg: Message) {
    let params = msg.params;
    if let Some(cwd) = params.get("cwd") {
        if let Some(dir) = cwd.as_str() {
            let result = match read_dir_entries(&dir) {
                Ok(entries) => {
                    let result = json!({
                    "data": entries,
                    "dir": dir,
                    "total": entries.len()}
                    );
                    json!({ "result": result, "id": msg.id })
                }
                Err(err) => json!({ "error": format!("{}:{}", dir, err), "id": msg.id }),
            };
            let s = serde_json::to_string(&result).expect("I promise; qed");
            println!("Content-length: {}\n\n{}", s.len(), s);
        }
    }
}

pub fn loop_handle_message(rx: &crossbeam_channel::Receiver<String>) {
    for msg in rx.iter() {
        thread::spawn(move || {
            // Ignore the invalid message.
            if let Ok(msg) = serde_json::from_str::<Message>(&msg.trim()) {
                match &msg.method[..] {
                    "filer" => handle_filer(msg),
                    _ => println!("{}", json!({ "error": "unknown method" })),
                }
            }
        });
    }
}

pub fn run<R>(reader: R)
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

fn into_string(entry: std::fs::DirEntry) -> String {
    if entry.path().is_dir() {
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
    }
}

fn read_dir_entries(dir: &str) -> Result<Vec<String>> {
    let mut entries = fs::read_dir(dir)?
        .map(|res| res.map(into_string))
        .collect::<Result<Vec<_>, io::Error>>()?;

    entries.sort();

    Ok(entries)
}

#[test]
fn test_dir() {
    let entries = read_dir_entries("/home/xlc/.vim/plugged/vim-clap").unwrap();
    println!("entry: {:?}", entries);
}
