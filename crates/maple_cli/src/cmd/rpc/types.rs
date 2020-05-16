use super::{write_response, Message};
use anyhow::Context;
use serde_json::json;
use std::convert::{TryFrom, TryInto};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct GrepPreviewEntry {
    pub fpath: std::path::PathBuf,
    pub lnum: u64,
    pub col: u64,
}

impl TryFrom<String> for GrepPreviewEntry {
    type Error = anyhow::Error;
    fn try_from(line: String) -> std::result::Result<Self, Self::Error> {
        lazy_static::lazy_static! {
            static ref GREP_RE: regex::Regex = regex::Regex::new(r"^(.*):(\d+):(\d+):").unwrap();
        }
        let cap = GREP_RE.captures(&line).context("Couldn't get captures")?;
        let fpath = cap
            .get(1)
            .map(|x| x.as_str().into())
            .context("Couldn't get fpath")?;
        let str2nr = |idx: usize| {
            cap.get(idx)
                .map(|x| x.as_str())
                .map(|x| x.parse::<u64>().expect("\\d+ matched"))
                .context("Couldn't parse u64")
        };
        let lnum = str2nr(2)?;
        let col = str2nr(3)?;
        Ok(Self { fpath, lnum, col })
    }
}

pub enum Provider {
    Files(PathBuf),
    Grep(GrepPreviewEntry),
}

impl TryFrom<Message> for Provider {
    type Error = anyhow::Error;
    fn try_from(msg: Message) -> std::result::Result<Self, Self::Error> {
        let provider_id = msg
            .params
            .get("provider_id")
            .and_then(|x| x.as_str())
            .unwrap_or("Unknown provider id");

        let cwd = String::from(
            msg.params
                .get("cwd")
                .and_then(|x| x.as_str())
                .unwrap_or("Missing cwd when deserializing into FilerParams"),
        );

        let fname = String::from(
            msg.params
                .get("curline")
                .and_then(|x| x.as_str())
                .unwrap_or("Missing fname when deserializing into FilerParams"),
        );

        match provider_id {
            "files" => {
                let mut fpath: PathBuf = cwd.into();
                fpath.push(&fname);
                Ok(Self::Files(fpath))
            }
            "grep" => {
                let preview_entry: GrepPreviewEntry = fname.try_into()?;
                Ok(Self::Grep(preview_entry))
            }
            _ => Err(anyhow::anyhow!("Couldn't into Provider")),
        }
    }
}

#[test]
fn test_grep_regex() {
    use std::convert::TryInto;
    let re = regex::Regex::new(r"^(.*):(\d+):(\d+):").unwrap();
    let line = "install.sh:1:5:#!/usr/bin/env bash";
    let e: GrepPreviewEntry = String::from(line).try_into().unwrap();
    println!("{:?}", e);
    // println!("{:?}", re.captures(line).and_then(|cap|cap.get(1)).and_then(|line|));
}
