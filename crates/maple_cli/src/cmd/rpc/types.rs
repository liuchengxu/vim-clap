use super::Message;
use anyhow::anyhow;
use anyhow::Context;
use lazy_static::lazy_static;
use std::convert::{TryFrom, TryInto};
use std::path::PathBuf;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GrepPreviewEntry {
    pub fpath: PathBuf,
    pub lnum: usize,
    pub col: usize,
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
                .map(|x| x.parse::<usize>().expect("\\d+ matched"))
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
    pub size: usize,
    pub provider: Provider,
}

pub enum Provider {
    Files(PathBuf),
    Filer { path: PathBuf, enable_icon: bool },
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

        let enable_icon = msg
            .params
            .get("enable_icon")
            .and_then(|x| x.as_bool())
            .unwrap_or(false);

        let cwd = String::from(
            msg.params
                .get("cwd")
                .and_then(|x| x.as_str())
                .unwrap_or("Missing cwd when deserializing into FilerParams"),
        );

        let fname_with_icon = String::from(
            msg.params
                .get("curline")
                .and_then(|x| x.as_str())
                .unwrap_or("Missing fname when deserializing into FilerParams"),
        );

        let fname = if enable_icon {
            fname_with_icon.chars().skip(2).collect()
        } else {
            fname_with_icon
        };

        let size = msg
            .params
            .get("preview_size")
            .and_then(|x| x.as_u64().map(|x| x as usize))
            .unwrap_or(5);

        let provider = match provider_id {
            "files" => {
                let mut fpath: PathBuf = cwd.into();
                fpath.push(&fname);
                Provider::Files(fpath)
            }
            "filer" => {
                let mut path: PathBuf = cwd.into();
                path.push(&fname);
                let enable_icon = msg
                    .params
                    .get("enable_icon")
                    .and_then(|x| x.as_bool())
                    .unwrap_or(false);
                Provider::Filer { path, enable_icon }
            }
            "grep" | "grep2" => {
                let mut preview_entry: GrepPreviewEntry = fname.try_into()?;
                let mut with_cwd: PathBuf = cwd.into();
                with_cwd.push(&preview_entry.fpath);
                preview_entry.fpath = with_cwd;
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
