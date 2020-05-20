use super::Message;
use anyhow::anyhow;
use anyhow::Context;
use lazy_static::lazy_static;
use std::convert::{TryFrom, TryInto};
use std::path::PathBuf;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GrepPreviewEntry {
    pub fpath: PathBuf,
    pub lnum: u64,
    pub col: u64,
}

impl TryFrom<String> for GrepPreviewEntry {
    type Error = anyhow::Error;
    fn try_from(line: String) -> std::result::Result<Self, Self::Error> {
        lazy_static! {
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

/// Preview environment on Vim CursorMoved event.
pub struct PreviewEnv {
    /// Number of lines to preview.
    pub size: u64,
    pub provider: Provider,
}

pub enum Provider {
    Files(PathBuf),
    Grep(GrepPreviewEntry),
}

impl TryFrom<Message> for PreviewEnv {
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

        let size = msg
            .params
            .get("preview_size")
            .and_then(|x| x.as_u64())
            .unwrap_or(5);

        let provider = match provider_id {
            "files" => {
                let mut fpath: PathBuf = cwd.into();
                fpath.push(&fname);
                Provider::Files(fpath)
            }
            "grep" | "grep2" => {
                let preview_entry: GrepPreviewEntry = fname.try_into()?;
                Provider::Grep(preview_entry)
            }
            _ => {
                return Err(anyhow!(
                    "Couldn't into PreviewEnv from Message: {:?}, unknown provider_id: {}",
                    msg,
                    provider_id
                ))
            }
        };

        Ok(Self { size, provider })
    }
}

#[test]
fn test_grep_regex() {
    use std::convert::TryInto;
    let re = regex::Regex::new(r"^(.*):(\d+):(\d+):").unwrap();
    let line = "install.sh:1:5:#!/usr/bin/env bash";
    let e: GrepPreviewEntry = String::from(line).try_into().unwrap();
    assert_eq!(
        e,
        GrepPreviewEntry {
            fpath: "install.sh".into(),
            lnum: 1,
            col: 5
        }
    );
}
