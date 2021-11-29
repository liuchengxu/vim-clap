use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::{anyhow, Context, Result};
use once_cell::sync::Lazy;
use serde_json::json;

use pattern::*;

use crate::previewer::{self, vim_help::HelpTagPreview};
use crate::stdio_server::{filer, global, session::SessionContext, write_response, MethodCall};
use crate::utils::build_abs_path;

static IS_FERESHING_CACHE: Lazy<AtomicBool> = Lazy::new(|| AtomicBool::new(false));

/// We want to preview a certain line in a file.
#[derive(Debug, Clone)]
pub struct CertainLine {
    pub path: PathBuf,
    pub lnum: usize,
}

impl CertainLine {
    pub fn new(path: PathBuf, lnum: usize) -> Self {
        Self { path, lnum }
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
    Grep(CertainLine),
    BLines(CertainLine),
    ProjTags(CertainLine),
    BufferTags(CertainLine),
    HelpTags {
        subject: String,
        doc_filename: String,
        runtimepath: String,
    },
}

impl OnMove {
    pub fn new(curline: String, context: &SessionContext) -> Result<(Self, Option<String>)> {
        let mut line_content = None;
        let context = match context.provider_id.as_str() {
            "filer" => unreachable!("filer has been handled ahead"),

            "files" | "git_files" => Self::Files(build_abs_path(&context.cwd, &curline)),
            "recent_files" => Self::Files(PathBuf::from(&curline)),
            "history" => {
                if curline.starts_with('~') {
                    Self::History(crate::utils::expand_tilde(curline)?)
                } else {
                    Self::History(build_abs_path(&context.cwd, &curline))
                }
            }
            "proj_tags" => {
                let (lnum, p) =
                    extract_proj_tags(&curline).context("Couldn't extract proj tags")?;
                let mut path: PathBuf = context.cwd.clone();
                path.push(&p);
                Self::ProjTags(CertainLine::new(path, lnum))
            }
            "coc_location" | "grep" | "grep2" => {
                let mut try_extract_file_path = |line: &str| {
                    let (fpath, lnum, _col, expected_line) =
                        extract_grep_position(line).context("Couldn't extract grep position")?;

                    let fpath = if let Ok(stripped) = fpath.strip_prefix("./") {
                        stripped.to_path_buf()
                    } else {
                        fpath
                    };

                    line_content = Some(expected_line.into());

                    let mut path: PathBuf = context.cwd.clone();
                    path.push(&fpath);
                    Ok::<(PathBuf, usize), anyhow::Error>((path, lnum))
                };

                let (path, lnum) = try_extract_file_path(&curline)?;

                Self::Grep(CertainLine::new(path, lnum))
            }
            "dumb_jump" => {
                let (_def_kind, fpath, lnum, _col) =
                    extract_jump_line_info(&curline).context("Couldn't extract jump line info")?;
                let mut path: PathBuf = context.cwd.clone();
                path.push(&fpath);
                Self::Grep(CertainLine::new(path, lnum))
            }
            "blines" => {
                let lnum = extract_blines_lnum(&curline).context("Couldn't extract buffer lnum")?;
                let path = context.start_buffer_path.clone();
                Self::BLines(CertainLine::new(path, lnum))
            }
            "tags" => {
                let lnum =
                    extract_buf_tags_lnum(&curline).context("Couldn't extract buffer tags")?;
                let path = context.start_buffer_path.clone();
                Self::BufferTags(CertainLine::new(path, lnum))
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
                let rev = parse_rev(&curline).context("Couldn't extract rev")?;
                Self::Commit(rev.into())
            }
            _ => {
                return Err(anyhow!(
                    "Couldn't construct an `OnMove` instance, you probably forget to add an implementation for this provider: {:?}",
                    context.provider_id
                ))
            }
        };

        Ok((context, line_content))
    }
}

pub struct OnMoveHandler<'a> {
    pub msg_id: u64,
    pub size: usize,
    pub inner: OnMove,
    pub context: &'a SessionContext,
    pub expected_line: Option<String>,
}

impl<'a> OnMoveHandler<'a> {
    pub fn create(
        msg: &MethodCall,
        context: &'a SessionContext,
        curline: Option<String>,
    ) -> anyhow::Result<Self> {
        let msg_id = msg.id;
        let curline = match curline {
            Some(line) => line,
            None => msg.get_curline(&context.provider_id)?,
        };
        if context.provider_id.as_str() == "filer" {
            let path = build_abs_path(&msg.get_cwd(), curline);
            return Ok(Self {
                msg_id,
                size: context.sensible_preview_size(),
                context,
                inner: OnMove::Filer(path),
                expected_line: None,
            });
        }
        let (inner, expected_line) = OnMove::new(curline, context)?;
        Ok(Self {
            msg_id,
            size: context.sensible_preview_size(),
            context,
            inner,
            expected_line,
        })
    }

    pub fn handle(&self) -> Result<()> {
        use OnMove::*;
        match &self.inner {
            BLines(CertainLine { path, lnum })
            | Grep(CertainLine { path, lnum })
            | ProjTags(CertainLine { path, lnum })
            | BufferTags(CertainLine { path, lnum }) => {
                self.preview_file_at(path, *lnum);
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
        let provider_id = &self.context.provider_id;
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
        let preview_tag = HelpTagPreview::new(subject, doc_filename, runtimepath);
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
            tracing::debug!(?preview_tag, "Can not find the preview help lines");
        }
    }

    fn try_refresh_cache(&self, highlight_line: &str) {
        if IS_FERESHING_CACHE.load(Ordering::Relaxed) {
            tracing::debug!(
                "Skipping the cache refreshing as there is already one that is running or waitting"
            );
            return;
        }
        if self.context.provider_id.as_str() == "grep2" {
            if let Some(ref expected) = self.expected_line {
                if !expected.eq(highlight_line) {
                    tracing::debug!(?expected, got = ?highlight_line, "The cache might be oudated");
                    let dir = self.context.cwd.clone();
                    IS_FERESHING_CACHE.store(true, Ordering::Relaxed);
                    // Spawn a future in the background
                    tokio::spawn(async move {
                        tracing::debug!(?dir, "Attempting to refresh grep2 cache");
                        match crate::command::grep::refresh_cache(dir) {
                            Ok(total) => {
                                tracing::debug!(total, "Refresh the grep2 cache successfully");
                            }
                            Err(e) => {
                                tracing::error!(error = ?e, "Failed to refresh the grep2 cache")
                            }
                        }
                        IS_FERESHING_CACHE.store(false, Ordering::Relaxed);
                    });
                }
            }
        }
    }

    fn preview_file_at(&self, path: &Path, lnum: usize) {
        tracing::debug!(?path, lnum, "Previewing file");

        match utility::read_preview_lines(path, lnum, self.size) {
            Ok((lines_iter, hi_lnum)) => {
                let fname = format!("{}", path.display());
                let lines = std::iter::once(format!("{}:{}", fname, lnum))
                    .chain(self.truncate_preview_lines(lines_iter.into_iter()))
                    .collect::<Vec<_>>();

                if let Some(got) = lines.get(hi_lnum) {
                    self.try_refresh_cache(got);
                }

                tracing::debug!(
                    msg_id = self.msg_id,
                    provider_id = %self.context.provider_id,
                    lines_len = lines.len(),
                    "<== message(out) sending event",
                );

                self.send_response(json!({
                  "event": "on_move",
                  "lines": lines,
                  "fname": fname,
                  "hi_lnum": hi_lnum
                }));
            }
            Err(err) => {
                tracing::error!(
                    ?path,
                    provider_id = %self.context.provider_id,
                    ?err,
                    "Couldn't read first lines",
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
        previewer::truncate_preview_lines(self.max_width(), lines)
    }

    /// Returns the maximum line width.
    fn max_width(&self) -> usize {
        2 * self.context.display_winwidth as usize
    }

    fn preview_file<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let (lines, fname) = previewer::preview_file(path, 2 * self.size, self.max_width())?;
        self.send_response(json!({
          "event": "on_move",
          "lines": lines,
          "fname": fname
        }));
        Ok(())
    }

    fn preview_directory<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let enable_icon = global().enable_icon;
        let lines = filer::read_dir_entries(&path, enable_icon, Some(2 * self.size))?;
        self.send_response(json!({
          "event": "on_move",
          "lines": lines,
          "is_dir": true
        }));
        Ok(())
    }
}
