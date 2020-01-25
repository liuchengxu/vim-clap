use anyhow::Result;
use crossbeam_channel::{bounded, unbounded, Receiver, Sender};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::io::prelude::*;
use std::thread;

/// Represents jsonrpc request which is a method call.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Message {
    /// A String containing the name of the method to be invoked.
    pub method: String,
    /// A Structured value that holds the parameter values to be used
    /// during the invocation of the method. This member MAY be omitted.
    pub params: serde_json::Map<String, Value>,
    /// An identifier established by the Client that MUST contain a String,
    /// Number, or NULL value if included. If it is not included it is assumed
    /// to be a notification.
    pub id: u64,
}

fn loop_read(reader: impl BufRead, sink: &Sender<String>) {
    let mut reader = reader;
    loop {
        let mut message = String::new();
        if reader.read_line(&mut message).unwrap() > 0 {
            // println!("----- readed {}", message);
            sink.send(message);
        }
    }
}

pub fn loop_call(rx: &crossbeam_channel::Receiver<String>) {
    for msg in rx.iter() {
        thread::spawn(move || {
            let msg = msg.trim();
            let msg: Message = serde_json::from_str(&msg).unwrap();
            // println!("method: {}----", msg.method);
            match &msg.method[..] {
                "open_file" => {
                    // println!("------ msg: {:?}", msg);
                    // println!("------ cwd: {:?}", msg.params.get("cwd"));
                    let dir = msg.params.get("cwd").unwrap().as_str().unwrap();
                    // println!("dir: {}", dir);
                    let json_msg = match read_entries(&dir) {
                        Ok(entries) => json!({ "data": entries, "dir": dir }),
                        Err(err) => json!({ "error": format!("{}:{}", dir, err) }),
                    };
                    // Warning:
                    //  Write multiple new line to ensure json_msg will not truncated by neovim.
                    // Not sure this is enough robust.
                    println!("\n\n{}", json_msg);
                    // let s = serde_json::to_string(&json_msg).expect("Fail to string");
                    // println!("Content-length: {}\n\n{}", s.len(), s);
                }
                _ => println!("{}", json!({ "error": "unknown method" })),
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
            // if let Err(err) = loop_read(reader) {
            // println!("Thread reader exited with error: {:?}", err);
            // }
        })
        .unwrap();
    loop_call(&rx);
}

fn read_entries(dir: &str) -> Result<Vec<String>> {
    use std::{fs, io};
    let mut entries = fs::read_dir(dir)?
        .map(|res| {
            res.map(|e| {
                if e.path().is_dir() {
                    format!(
                        "{}/",
                        e.path()
                            .file_name()
                            .and_then(std::ffi::OsStr::to_str)
                            .unwrap()
                    )
                } else {
                    e.path()
                        .file_name()
                        .and_then(std::ffi::OsStr::to_str)
                        .map(Into::into)
                        .unwrap()
                }
            })
        })
        .collect::<Result<Vec<_>, io::Error>>()?;

    // The order in which `read_dir` returns entries is not guaranteed. If reproducible
    // ordering is required the entries should be explicitly sorted.

    entries.sort();

    Ok(entries)
}

#[test]
fn test_dir() {
    let entries = read_entries("/home/xlc/.vim/plugged/vim-clap").unwrap();

    println!("entry: {:?}", entries);
    // The entries have now been sorted by their path.
}
