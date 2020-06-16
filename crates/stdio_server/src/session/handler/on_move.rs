use crate::session::SessionContext;
use crate::types::{Message, ProviderId};
use crate::write_response;
use anyhow::{anyhow, Context, Result};
use log::{debug, error};
use pattern::*;
use serde_json::json;
use std::path::Path;
use std::path::PathBuf;

#[inline]
fn as_absolute_path<P: AsRef<Path>>(path: P) -> Result<String> {
    std::fs::canonicalize(path.as_ref())?
        .into_os_string()
        .into_string()
        .map_err(|e| anyhow!("{:?}, path:{}", e, path.as_ref().display()))
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

/// Build the absolute path using cwd and relative path.
pub fn build_abs_path(cwd: &str, curline: String) -> PathBuf {
    let mut path: PathBuf = cwd.into();
    path.push(&curline);
    path
}

impl OnMove {
    pub fn new(curline: String, context: &SessionContext) -> Result<Self> {
        let context = match context.provider_id.as_str() {
            "files" | "git_files" => Self::Files(build_abs_path(&context.cwd, curline)),
            "filer" => unreachable!("filer has been handled ahead"),
            "proj_tags" => {
                let (lnum, p) =
                    extract_proj_tags(&curline).context("Couldn't extract proj tags")?;
                let mut path: PathBuf = context.cwd.clone().into();
                path.push(&p);
                Self::ProjTags { path, lnum }
            }
            "grep" | "grep2" => {
                let (fpath, lnum, _col) =
                    extract_grep_position(&curline).context("Couldn't extract grep position")?;
                let mut path: PathBuf = context.cwd.clone().into();
                path.push(&fpath);
                Self::Grep { path, lnum }
            }
            "blines" => {
                let lnum = extract_blines_lnum(&curline).context("Couldn't extract buffer lnum")?;
                let path = context.start_buffer_path.clone().into();
                Self::BLines { path, lnum }
            }
            "tags" => {
                let lnum =
                    extract_buf_tags_lnum(&curline).context("Couldn't extract buffer tags")?;
                let path = context.start_buffer_path.clone().into();
                Self::BufferTags { path, lnum }
            }
            _ => {
                return Err(anyhow!(
                    "Couldn't constructs a OnMove instance, context: {:?}",
                    context
                ))
            }
        };

        Ok(context)
    }
}

pub struct OnMoveHandler {
    pub msg_id: u64,
    pub provider_id: ProviderId,
    pub size: usize,
    pub inner: OnMove,
}

impl OnMoveHandler {
    pub fn try_new(msg: Message, context: &SessionContext) -> anyhow::Result<Self> {
        let msg_id = msg.id;
        let provider_id = context.provider_id.clone();
        let curline = msg.get_curline(&provider_id)?;
        if provider_id.as_str() == "filer" {
            let path = build_abs_path(
                &msg.get_cwd()
                    .ok_or(anyhow!("Missing cwd in message.params"))?,
                curline,
            );
            return Ok(Self {
                msg_id,
                size: provider_id.get_preview_size(),
                provider_id,
                inner: OnMove::Filer(path),
            });
        }
        Ok(Self {
            msg_id,
            size: provider_id.get_preview_size(),
            provider_id,
            inner: OnMove::new(curline, context)?,
        })
    }

    pub fn handle(&self) -> Result<()> {
        use OnMove::*;
        match &self.inner {
            BLines { path, lnum }
            | Grep { path, lnum }
            | ProjTags { path, lnum }
            | BufferTags { path, lnum } => {
                debug!("path:{}, lnum:{}", path.display(), lnum);
                self.preview_file_at(&path, *lnum);
            }
            Filer(path) if path.is_dir() => {
                self.preview_directory(&path)?;
            }
            Files(path) | Filer(path) => {
                self.preview_file(&path)?;
            }
        }

        Ok(())
    }

    fn send_response(&self, result: serde_json::value::Value) {
        let provider_id: crate::types::ProviderId = self.provider_id.clone().into();
        write_response(json!({
                "id": self.msg_id,
                "provider_id": provider_id,
                "result": result
        }));
    }

    fn preview_file_at<P: AsRef<Path>>(&self, path: P, lnum: usize) {
        match utility::read_preview_lines(path.as_ref(), lnum, self.size) {
            Ok((lines_iter, hi_lnum)) => {
                let fname = format!("{}", path.as_ref().display());
                let lines = std::iter::once(fname.clone())
                    .chain(lines_iter)
                    .collect::<Vec<_>>();
                debug!(
                    "sending msg_id:{}, provider_id:{}",
                    self.msg_id, self.provider_id
                );
                self.send_response(json!({
                  "event": "on_move",
                  "lines": lines,
                  "fname": fname,
                  "hi_lnum": hi_lnum
                }));
            }
            Err(err) => {
                error!(
                    "[{}]Couldn't read first lines of {}, error: {:?}",
                    self.provider_id,
                    path.as_ref().display(),
                    err
                );
            }
        }
    }

    fn preview_file<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let abs_path = as_absolute_path(path.as_ref())?;
        let lines_iter = utility::read_first_lines(path.as_ref(), 2 * self.size)?;
        let lines = std::iter::once(abs_path.clone())
            .chain(lines_iter)
            .collect::<Vec<_>>();
        self.send_response(json!({
          "event": "on_move",
          "lines": lines,
          "fname": abs_path
        }));
        Ok(())
    }

    fn preview_directory<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let enable_icon = crate::env::global().enable_icon;
        let lines = crate::filer::read_dir_entries(&path, enable_icon, Some(2 * self.size))?;
        self.send_response(json!({
          "event": "on_move",
          "lines": lines,
          "is_dir": true
        }));
        Ok(())
    }
}
