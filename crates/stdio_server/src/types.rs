use anyhow::anyhow;
use anyhow::Context;
use pattern::{
    extract_blines_lnum, extract_buf_tags_lnum, extract_grep_position, extract_proj_tags,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::convert::TryFrom;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct GlobalEnv {
    pub is_nvim: bool,
    pub enable_icon: bool,
    pub preview_size: Value,
}

impl GlobalEnv {
    pub fn new(is_nvim: bool, enable_icon: bool, preview_size: Value) -> Self {
        Self {
            is_nvim,
            enable_icon,
            preview_size,
        }
    }

    pub fn preview_size_of(&self, provider_id: &str) -> usize {
        match self.preview_size {
            Value::Number(ref number) => number.as_u64().unwrap() as usize,
            Value::Object(ref obj) => {
                let get_size = |key: &str| {
                    obj.get(key)
                        .and_then(|x| x.as_u64().map(|i| i as usize))
                        .unwrap()
                };
                if obj.contains_key(provider_id) {
                    get_size(provider_id)
                } else if obj.contains_key("*") {
                    get_size("*")
                } else {
                    5usize
                }
            }
            _ => unreachable!("clap_preview_size has to be either Number or Object"),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Message {
    pub method: String,
    pub params: serde_json::Map<String, Value>,
    pub id: u64,
    pub session_id: u64,
}

impl Message {
    pub fn get_message_id(&self) -> u64 {
        self.id
    }

    pub fn get_provider_id(&self) -> String {
        self.params
            .get("provider_id")
            .and_then(|x| x.as_str())
            .unwrap_or("Unknown provider id")
            .into()
    }

    #[allow(dead_code)]
    pub fn get_query(&self) -> String {
        self.params
            .get("query")
            .and_then(|x| x.as_str())
            .expect("Unknown provider id")
            .into()
    }
}

/// Preview environment on Vim CursorMoved event.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum OnMove {
    Files(PathBuf),
    Filer(PathBuf),
    Grep { path: PathBuf, lnum: usize },
    BLines { path: PathBuf, lnum: usize },
    ProjTags { path: PathBuf, lnum: usize },
    BufferTags { path: PathBuf, lnum: usize },
}

impl TryFrom<Message> for OnMove {
    type Error = anyhow::Error;
    fn try_from(msg: Message) -> std::result::Result<Self, Self::Error> {
        let provider_id = msg
            .params
            .get("provider_id")
            .and_then(|x| x.as_str())
            .context("Missing provider id")?;

        let cwd = msg
            .params
            .get("cwd")
            .and_then(|x| x.as_str())
            .context("Missing cwd when deserializing into FilerParams")?;

        let display_curline = String::from(
            msg.params
                .get("curline")
                .and_then(|x| x.as_str())
                .context("Missing fname when deserializing into FilerParams")?,
        );

        let curline = if super::env::should_skip_leading_icon(provider_id) {
            display_curline.chars().skip(2).collect()
        } else {
            display_curline
        };

        let get_source_fpath = || {
            msg.params
                .get("source_fpath")
                .and_then(|x| x.as_str().map(Into::into))
                .context("Missing source_fpath")
        };

        // Rebuild the absolute path using cwd and relative path.
        let rebuild_abs_path = || {
            let mut path: PathBuf = cwd.into();
            path.push(&curline);
            path
        };

        log::debug!("curline: {}", curline);
        let context = match provider_id {
            "files" | "git_files" => Self::Files(rebuild_abs_path()),
            "filer" => Self::Filer(rebuild_abs_path()),
            "blines" => {
                let lnum = extract_blines_lnum(&curline).context("Couldn't extract buffer lnum")?;
                let path = get_source_fpath()?;
                Self::BLines { path, lnum }
            }
            "tags" => {
                let lnum =
                    extract_buf_tags_lnum(&curline).context("Couldn't extract buffer tags")?;
                let path = get_source_fpath()?;
                Self::BufferTags { path, lnum }
            }
            "proj_tags" => {
                let (lnum, p) =
                    extract_proj_tags(&curline).context("Couldn't extract proj tags")?;
                let mut path: PathBuf = cwd.into();
                path.push(&p);
                Self::ProjTags { path, lnum }
            }
            "grep" | "grep2" => {
                let (fpath, lnum, _col) =
                    extract_grep_position(&curline).context("Couldn't extract grep position")?;
                let mut path: PathBuf = cwd.into();
                path.push(&fpath);
                Self::Grep { path, lnum }
            }
            _ => {
                return Err(anyhow!(
                    "Couldn't into PreviewEnv from Message: {:?}, unknown provider_id: {}",
                    msg,
                    provider_id
                ))
            }
        };

        Ok(context)
    }
}
