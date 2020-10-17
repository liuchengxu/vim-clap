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
pub fn as_absolute_path<P: AsRef<Path>>(path: P) -> Result<String> {
    std::fs::canonicalize(path.as_ref())?
        .into_os_string()
        .into_string()
        .map_err(|e| anyhow!("{:?}, path:{}", e, path.as_ref().display()))
}

/// Preview environment on Vim CursorMoved event.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum OnMove {
    Commit(String),
    Files(PathBuf),
    Filer(PathBuf),
    History(PathBuf),
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
            "history" => {
                if curline.starts_with('~') {
                    // I know std::env::home_dir() is incorrect in some rare cases[1], but dirs crate has been archived.
                    //
                    // [1] https://www.reddit.com/r/rust/comments/ga7f56/why_dirs_and_directories_repositories_have_been/fsjbsac/
                    #[allow(deprecated)]
                    let mut path = std::env::home_dir().context("failed to get home_dir")?;
                    path.push(&curline[2..]);
                    Self::History(path)
                } else {
                    Self::History(build_abs_path(&context.cwd, curline))
                }
            }
            "filer" => unreachable!("filer has been handled ahead"),
            "proj_tags" => {
                let (lnum, p) = extract_proj_tags(&curline).context("can not extract proj tags")?;
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
                let lnum = extract_blines_lnum(&curline).context("can not extract buffer lnum")?;
                let path = context.start_buffer_path.clone().into();
                Self::BLines { path, lnum }
            }
            "tags" => {
                let lnum =
                    extract_buf_tags_lnum(&curline).context("can not extract buffer tags")?;
                let path = context.start_buffer_path.clone().into();
                Self::BufferTags { path, lnum }
            }
            "commits" | "bcommits" => {
                let rev = parse_rev(&curline).context("can not extract rev")?;
                Self::Commit(rev.into())
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

pub struct OnMoveHandler<'a> {
    pub msg_id: u64,
    pub provider_id: ProviderId,
    pub size: usize,
    pub inner: OnMove,
    pub context: &'a SessionContext,
}

impl<'a> OnMoveHandler<'a> {
    pub fn try_new(msg: Message, context: &'a SessionContext) -> anyhow::Result<Self> {
        let msg_id = msg.id;
        let provider_id = context.provider_id.clone();
        let curline = msg.get_curline(&provider_id)?;
        if provider_id.as_str() == "filer" {
            let path = build_abs_path(&msg.get_cwd(), curline);
            return Ok(Self {
                msg_id,
                size: provider_id.get_preview_size(),
                provider_id,
                context,
                inner: OnMove::Filer(path),
            });
        }
        Ok(Self {
            msg_id,
            size: provider_id.get_preview_size(),
            provider_id,
            context,
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
            Files(path) | Filer(path) | History(path) => {
                self.preview_file(&path)?;
            }
            Commit(rev) => {
                self.show_commit(rev)?;
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

    fn show_commit(&self, rev: &str) -> Result<()> {
        let stdout = self.context.execute(&format!("git show {}", rev))?;
        let stdout_str = String::from_utf8_lossy(&stdout);
        let lines = stdout_str
            .split('\n')
            .take(self.provider_id.get_preview_size() * 2)
            .collect::<Vec<_>>();
        self.send_response(json!({
          "event": "on_move",
          "lines": lines,
        }));
        Ok(())
    }

    fn preview_file_at<P: AsRef<Path>>(&self, path: P, lnum: usize) {
        match utility::read_preview_lines(path.as_ref(), lnum, self.size) {
            Ok((lines_iter, hi_lnum)) => {
                let fname = format!("{}", path.as_ref().display());
                let lines = std::iter::once(fname.clone())
                    .chain(self.truncate_preview_lines(lines_iter))
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

    /// Truncates the lines that are awfully long as vim can not handle them properly.
    ///
    /// Ref https://github.com/liuchengxu/vim-clap/issues/543
    fn truncate_preview_lines(
        &self,
        lines: impl Iterator<Item = String>,
    ) -> impl Iterator<Item = String> {
        let max_width = 2 * self.context.winwidth.unwrap_or(100) as usize;
        lines.map(move |line| {
            if line.len() > max_width {
                let mut line = line;
                // https://github.com/liuchengxu/vim-clap/pull/544#discussion_r506281014
                line.truncate(
                    (0..max_width + 1)
                        .rev()
                        .find(|idx| line.is_char_boundary(*idx))
                        .unwrap(),
                );
                line.push_str("……");
                line
            } else {
                line
            }
        })
    }

    fn preview_file<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let abs_path = as_absolute_path(path.as_ref())?;
        let lines_iter = utility::read_first_lines(path.as_ref(), 2 * self.size)?;
        let lines = std::iter::once(abs_path.clone())
            .chain(self.truncate_preview_lines(lines_iter))
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
