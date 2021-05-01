use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use log::{debug, error};
use serde_json::json;

use pattern::*;

use crate::stdio_server::{
    session::SessionContext,
    types::{Message, ProviderId},
    write_response,
};

#[inline]
pub fn as_absolute_path<P: AsRef<Path>>(path: P) -> Result<String> {
    std::fs::canonicalize(path.as_ref())?
        .into_os_string()
        .into_string()
        .map_err(|e| anyhow!("{:?}, path:{}", e, path.as_ref().display()))
}

#[derive(Debug, Clone)]
struct PreviewTag {
    subject: String,
    doc_filename: String,
    runtimepath: String,
}

fn find_tag_line(p: &Path, subject: &str) -> Option<usize> {
    if let Ok(doc_lines) = utility::read_lines(p) {
        for (idx, doc_line) in doc_lines.enumerate() {
            if let Ok(d_line) = doc_line {
                if d_line.trim().contains(&format!("*{}*", subject)) {
                    return Some(idx);
                }
            }
        }
    }
    None
}

impl PreviewTag {
    pub fn new(subject: String, doc_filename: String, runtimepath: String) -> Self {
        Self {
            subject,
            doc_filename,
            runtimepath,
        }
    }

    pub fn get_help_lines(&self, size: usize) -> Option<(String, Vec<String>)> {
        for r in self.runtimepath.split(',') {
            let p = Path::new(r).join("doc").join(&self.doc_filename);
            if p.exists() {
                if let Some(line_number) = find_tag_line(&p, &self.subject) {
                    if let Ok(lines_iter) = utility::read_lines_from(&p, line_number, size) {
                        return Some((format!("{}", p.display()), lines_iter.collect()));
                    }
                }
            }
        }

        None
    }
}

/// Preview environment on Vim CursorMoved event.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum OnMove {
    Commit(String),
    Files(PathBuf),
    Filer(PathBuf),
    History(PathBuf),
    Grep {
        path: PathBuf,
        lnum: usize,
    },
    BLines {
        path: PathBuf,
        lnum: usize,
    },
    ProjTags {
        path: PathBuf,
        lnum: usize,
    },
    BufferTags {
        path: PathBuf,
        lnum: usize,
    },
    HelpTags {
        subject: String,
        doc_filename: String,
        runtimepath: String,
    },
}

/// Build the absolute path using cwd and relative path.
pub fn build_abs_path<P: AsRef<Path>>(cwd: P, curline: String) -> PathBuf {
    let mut path: PathBuf = cwd.as_ref().into();
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
                let mut path: PathBuf = context.cwd.clone();
                path.push(&p);
                Self::ProjTags { path, lnum }
            }
            "grep" | "grep2" => {
                let try_extract_file_path = |line: &str| {
                    let (fpath, lnum, _col) =
                        extract_grep_position(line).context("Couldn't extract grep position")?;
                    let mut path: PathBuf = context.cwd.clone();
                    path.push(&fpath);
                    Ok::<(PathBuf, usize), anyhow::Error>((path, lnum))
                };

                let (path, lnum) = try_extract_file_path(&curline)?;

                Self::Grep { path, lnum }
            }
            "dumb_jump" => {
                let (_def_kind, fpath, lnum, _col) =
                    extract_jump_line_info(&curline).context("Couldn't extract jump line info")?;
                let mut path: PathBuf = context.cwd.clone();
                path.push(&fpath);
                Self::Grep { path, lnum }
            }
            "blines" => {
                let lnum = extract_blines_lnum(&curline).context("can not extract buffer lnum")?;
                let path = context.start_buffer_path.clone();
                Self::BLines { path, lnum }
            }
            "tags" => {
                let lnum =
                    extract_buf_tags_lnum(&curline).context("can not extract buffer tags")?;
                let path = context.start_buffer_path.clone();
                Self::BufferTags { path, lnum }
            }
            "help_tags" => {
                let runtimepath = context
                    .runtimepath
                    .clone()
                    .context("no runtimepath in the context")?;
                let items = curline.split('\t').collect::<Vec<_>>();
                if items.len() < 2 {
                    return Err(anyhow!(
                        "Can not extract subject and doc_filename from the line"
                    ));
                }
                Self::HelpTags {
                    subject: items[0].trim().to_string(),
                    doc_filename: items[1].trim().to_string(),
                    runtimepath,
                }
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
    pub fn try_new(msg: &Message, context: &'a SessionContext) -> anyhow::Result<Self> {
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
        let size = std::cmp::max(
            provider_id.get_preview_size(),
            (context.preview_winheight / 2) as usize,
        );
        Ok(Self {
            msg_id,
            size,
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
                self.preview_file_at(&path, *lnum);
            }
            HelpTags {
                subject,
                doc_filename,
                runtimepath,
            } => self.preview_help_subject(subject, doc_filename, runtimepath),
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
        let provider_id: ProviderId = self.provider_id.clone();
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
            .take(self.size * 2)
            .collect::<Vec<_>>();
        self.send_response(json!({
          "event": "on_move",
          "lines": lines,
        }));
        Ok(())
    }

    fn preview_help_subject(&self, subject: &str, doc_filename: &str, runtimepath: &str) {
        let preview_tag = PreviewTag::new(subject.into(), doc_filename.into(), runtimepath.into());
        if let Some((fname, lines)) = preview_tag.get_help_lines(self.size * 2) {
            let lines = std::iter::once(fname.clone())
                .chain(lines.into_iter())
                .collect::<Vec<_>>();
            self.send_response(json!({
              "event": "on_move",
              "syntax": "help",
              "lines": lines,
              "hi_lnum": 1,
              "fname": fname
            }));
        } else {
            debug!("Can not find the preview help lines for {:?}", preview_tag);
        }
    }

    fn preview_file_at<P: AsRef<Path>>(&self, path: P, lnum: usize) {
        debug!(
            "try to preview the file, path: {}, lnum: {}",
            path.as_ref().display(),
            lnum
        );

        match utility::read_preview_lines(path.as_ref(), lnum, self.size) {
            Ok((lines_iter, hi_lnum)) => {
                let fname = format!("{}", path.as_ref().display());
                let lines = std::iter::once(format!("{}:{}", fname, lnum))
                    .chain(self.truncate_preview_lines(lines_iter.into_iter()))
                    .collect::<Vec<_>>();
                debug!(
                    "<== message(out) sending event: on_move, msg_id:{}, provider_id:{}, lines: {:?}",
                    self.msg_id, self.provider_id, lines
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
        let max_width = 2 * self.context.display_winwidth as usize;
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
        let enable_icon = crate::stdio_server::global().enable_icon;
        let lines =
            crate::stdio_server::filer::read_dir_entries(&path, enable_icon, Some(2 * self.size))?;
        self.send_response(json!({
          "event": "on_move",
          "lines": lines,
          "is_dir": true
        }));
        Ok(())
    }
}
