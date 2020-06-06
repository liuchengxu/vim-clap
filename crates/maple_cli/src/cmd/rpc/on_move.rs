use super::types::{OnMove, OnMove::*};
use super::*;
use anyhow::{anyhow, Result};
use log::{debug, error};
use std::convert::{TryFrom, TryInto};
use std::path::Path;

#[inline]
fn as_absolute_path<P: AsRef<Path>>(path: P) -> Result<String> {
    std::fs::canonicalize(path.as_ref())?
        .into_os_string()
        .into_string()
        .map_err(|e| anyhow!("{:?}, path:{}", e, path.as_ref().display()))
}

pub struct OnMoveHandler {
    pub msg_id: u64,
    pub provider_id: String,
    pub size: usize,
    pub inner: OnMove,
}

impl TryFrom<Message> for OnMoveHandler {
    type Error = anyhow::Error;
    fn try_from(msg: Message) -> std::result::Result<Self, Self::Error> {
        let msg_id = msg.get_message_id();
        let provider_id = msg.get_provider_id();
        let size = super::env::preview_size_of(&provider_id);
        let inner: OnMove = msg.try_into()?;
        Ok(Self {
            msg_id,
            provider_id,
            size,
            inner,
        })
    }
}

impl OnMoveHandler {
    pub fn handle(&self) -> Result<()> {
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

    fn preview_file_at<P: AsRef<Path>>(&self, path: P, lnum: usize) {
        match crate::utils::read_preview_lines(path.as_ref(), lnum, self.size) {
            Ok((lines_iter, hi_lnum)) => {
                let fname = format!("{}", path.as_ref().display());
                let lines = std::iter::once(fname.clone())
                    .chain(lines_iter)
                    .collect::<Vec<_>>();
                debug!(
                    "sending msg_id:{}, provider_id:{}",
                    self.msg_id, self.provider_id
                );
                write_response(json!({
                "id": self.msg_id,
                "provider_id": self.provider_id,
                "result": {
                  "event": "on_move",
                  "lines": lines,
                  "fname": fname,
                  "hi_lnum": hi_lnum
                }}));
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
        let lines_iter = crate::utils::read_first_lines(path.as_ref(), 2 * self.size)?;
        let lines = std::iter::once(abs_path.clone())
            .chain(lines_iter)
            .collect::<Vec<_>>();
        write_response(json!({
        "id": self.msg_id,
        "provider_id": self.provider_id,
        "result": {
          "event": "on_move",
          "lines": lines,
          "fname": abs_path
        }}));
        Ok(())
    }

    fn preview_directory<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let enable_icon = super::env::global().enable_icon;
        let lines = super::filer::read_dir_entries(&path, enable_icon, Some(2 * self.size))?;
        write_response(json!({
        "id": self.msg_id,
        "provider_id": self.provider_id,
        "result": {
          "event": "on_move",
          "lines": lines,
          "is_dir": true
        }}));
        Ok(())
    }
}
