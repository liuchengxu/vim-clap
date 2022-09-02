use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{anyhow, Result};
use serde_json::{json, Value};

use pattern::*;
use types::PreviewInfo;

use crate::previewer;
use crate::previewer::vim_help::HelpTagPreview;
use crate::stdio_server::impls::providers::filer;
use crate::stdio_server::session::SessionContext;
use crate::stdio_server::{global, job};
use crate::tools::ctags::{current_context_tag_async, BufferTag};
use crate::utils::{build_abs_path, display_width, truncate_absolute_path};

#[derive(Debug)]
pub enum PreviewKind {
    /// Should be a file.
    File(PathBuf),
    /// Maybe a file or a directory.
    FileOrDirectory(PathBuf),
    /// A specific location in a file.
    Line { path: PathBuf, line_number: usize },
    /// Commit revision.
    Commit(String),
    HelpTags {
        subject: String,
        doc_filename: String,
        runtimepath: String,
    },
}

fn parse_preview_kind(
    curline: String,
    context: &SessionContext,
) -> Result<(PreviewKind, Option<String>)> {
    let err = || {
        anyhow!(
            "Failed to parse PreviewKind for provider_id: {} from `{curline}`",
            context.provider_id.as_str()
        )
    };

    // Store the line context we see in the search result, but it may be out-dated due to the
    // cacheh is being used, especially for the providers like grep which potentially have tons of
    // items.
    //
    // If the line we see mismatches the actual line content in the preview, the content in which
    // is always accurate, try to refresh the cache and reload.
    let mut line_content = None;

    let preview_kind = match context.provider_id.as_str() {
            "files" | "git_files" => PreviewKind::File(build_abs_path(&context.cwd, &curline)),
            "recent_files" => PreviewKind::File(PathBuf::from(&curline)),
            "history" => {
                let path = if curline.starts_with('~') {
                    crate::utils::expand_tilde(curline)
                } else {
                    build_abs_path(&context.cwd, &curline)
                };
                PreviewKind::File(path)
            }
            "coc_location" | "grep" | "grep2" => {
                let mut try_extract_file_path = |line: &str| {
                    let (fpath, lnum, _col, cache_line) = extract_grep_position(line).ok_or_else(err)?;

                    let fpath = if let Ok(stripped) = fpath.strip_prefix("./") {
                        stripped.to_path_buf()
                    } else {
                        fpath
                    };

                    line_content.replace(cache_line.into());

                    let mut path: PathBuf = context.cwd.clone();
                    path.push(&fpath);

                    Ok::<(PathBuf, usize), anyhow::Error>((path, lnum))
                };

                let (path, line_number) = try_extract_file_path(&curline)?;

                PreviewKind::Line{ path, line_number }
            }
            "dumb_jump" => {
                let (_def_kind, fpath, line_number, _col) = extract_jump_line_info(&curline).ok_or_else(err)?;
                let mut path: PathBuf = context.cwd.clone();
                path.push(&fpath);
                PreviewKind::Line{ path, line_number }
            }
            "blines" => {
                let line_number = extract_blines_lnum(&curline).ok_or_else(err)?;
                let path = context.start_buffer_path.clone();
                PreviewKind::Line { path, line_number }
            }
            "tags" => {
                let line_number = extract_buf_tags_lnum(&curline).ok_or_else(err)?;
                let path = context.start_buffer_path.clone();
                PreviewKind::Line{ path, line_number }
            }
            "proj_tags" => {
                let (line_number, p) = extract_proj_tags(&curline).ok_or_else(err)?;
                let mut path: PathBuf = context.cwd.clone();
                path.push(&p);
                PreviewKind::Line{path, line_number}
            }
            "commits" | "bcommits" => {
                let rev = parse_rev(&curline).ok_or_else(err)?;
                PreviewKind::Commit(rev.into())
            }
            _ => {
                return Err(anyhow!(
                    "Failed to parse PreviewKind, you probably forget to add an implementation for this provider: {:?}",
                    context.provider_id
                ))
            }
        };

    Ok((preview_kind, line_content))
}

/// Returns `true` if the file path of preview file should be truncateted relative to cwd.
fn should_truncate_cwd_relative(provider_id: &str) -> bool {
    const SET: &[&str] = &[
        "files",
        "git_files",
        "grep",
        "grep2",
        "coc_location",
        "proj_tags",
    ];
    SET.contains(&provider_id)
}

#[derive(Debug)]
pub struct OnMoveHandler<'a> {
    pub size: usize,
    pub context: &'a SessionContext,
    pub preview_kind: PreviewKind,
    /// When the line extracted from the cache mismatches the latest
    /// preview line content, which means the cache is outdated, we
    /// should refresh the cache.
    ///
    /// Currently only for the provider `grep2`.
    pub cache_line: Option<String>,
}

impl<'a> OnMoveHandler<'a> {
    pub fn create(
        curline: String,
        preview_size: usize,
        context: &'a SessionContext,
    ) -> Result<Self> {
        let (preview_kind, cache_line) = parse_preview_kind(curline, context)?;

        Ok(Self {
            size: preview_size,
            context,
            preview_kind,
            cache_line,
        })
    }

    pub async fn get_preview(&self) -> Result<Value> {
        let value = match &self.preview_kind {
            PreviewKind::FileOrDirectory(path) => {
                if path.is_dir() {
                    self.preview_directory(&path)?
                } else {
                    self.preview_file(&path)?
                }
            }
            PreviewKind::File(path) => self.preview_file(&path)?,
            PreviewKind::Line { path, line_number } => {
                self.preview_file_at(path, *line_number).await
            }
            PreviewKind::Commit(rev) => self.preview_commits(rev)?,
            PreviewKind::HelpTags {
                subject,
                doc_filename,
                runtimepath,
            } => self.preview_help_subject(subject, doc_filename, runtimepath),
        };

        Ok(value)
    }

    fn preview_commits(&self, rev: &str) -> std::io::Result<Value> {
        let stdout = self.context.execute(&format!("git show {rev}"))?;
        let stdout_str = String::from_utf8_lossy(&stdout);
        let lines = stdout_str
            .split('\n')
            .take(self.size * 2)
            .collect::<Vec<_>>();
        Ok(json!({ "lines": lines }))
    }

    fn preview_help_subject(&self, subject: &str, doc_filename: &str, runtimepath: &str) -> Value {
        let preview_tag = HelpTagPreview::new(subject, doc_filename, runtimepath);
        if let Some((fname, lines)) = preview_tag.get_help_lines(self.size * 2) {
            let lines = std::iter::once(fname.clone())
                .chain(lines.into_iter())
                .collect::<Vec<_>>();
            json!({ "syntax": "help", "lines": lines, "hi_lnum": 1, "fname": fname })
        } else {
            tracing::debug!(?preview_tag, "Can not find the preview help lines");
            json!({ "lines": vec!["Can not find the preview help lines"] })
        }
    }

    fn preview_directory<P: AsRef<Path>>(&self, path: P) -> std::io::Result<Value> {
        let enable_icon = global().enable_icon;
        let lines = filer::read_dir_entries(&path, enable_icon, Some(2 * self.size))?;
        Ok(json!({ "lines": lines, "is_dir": true }))
    }

    fn preview_file<P: AsRef<Path>>(&self, path: P) -> std::io::Result<Value> {
        if !path.as_ref().is_file() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!(
                    "Faile to preview as {} is not a file",
                    path.as_ref().display()
                ),
            ));
        }

        let handle_io_error = |e: &std::io::Error| {
            if e.kind() == std::io::ErrorKind::NotFound {
                tracing::debug!(
                    "TODO: {} not found, the files cache might be invalid, try refreshing the cache",
                    path.as_ref().display()
                );
            }
        };

        let (lines, fname) = if !global().is_nvim {
            let (lines, abs_path) =
                previewer::preview_file(path.as_ref(), 2 * self.size, self.max_line_width())
                    .map_err(|e| {
                        handle_io_error(&e);
                        e
                    })?;
            let cwd = self.context.cwd.to_str().expect("Cwd is valid");
            // cwd is shown via the popup title, no need to include it again.
            let cwd_relative = abs_path.replacen(cwd, ".", 1);
            let mut lines = lines;
            lines[0] = cwd_relative;
            (lines, abs_path)
        } else {
            let max_fname_len = self.context.display_winwidth as usize - 1;
            previewer::preview_file_with_truncated_title(
                path.as_ref(),
                2 * self.size,
                self.max_line_width(),
                max_fname_len,
            )
            .map_err(|e| {
                handle_io_error(&e);
                e
            })?
        };

        if let Some(syntax) = crate::stdio_server::vim::syntax_for(path.as_ref()) {
            Ok(json!({ "lines": lines, "syntax": syntax }))
        } else {
            Ok(json!({ "lines": lines, "fname": fname }))
        }
    }

    async fn preview_file_at(&self, path: &Path, lnum: usize) -> Value {
        tracing::debug!(path=?path.display(), lnum, "Previewing file");

        let container_width = self.context.display_winwidth as usize;
        let fname = path.display().to_string();

        let truncated_preview_header = || {
            if !global().is_nvim && should_truncate_cwd_relative(self.context.provider_id.as_str())
            {
                // cwd is shown via the popup title, no need to include it again.
                let cwd_relative =
                    fname.replacen(self.context.cwd.to_str().expect("Cwd is valid"), ".", 1);
                format!("{cwd_relative}:{lnum}")
            } else {
                let max_fname_len = container_width - 1 - display_width(lnum);
                let truncated_abs_path = truncate_absolute_path(&fname, max_fname_len);
                format!("{truncated_abs_path}:{lnum}")
            }
        };

        match utility::read_preview_lines(path, lnum, self.size) {
            Ok(PreviewInfo {
                lines,
                highlight_lnum,
                start,
                ..
            }) => {
                let mut context_lines = Vec::new();

                // Some checks against the latest preview line.
                if let Some(latest_line) = lines.get(highlight_lnum - 1) {
                    self.try_refresh_cache(latest_line);

                    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                        const BLACK_LIST: &[&str] =
                            &["log", "txt", "lock", "toml", "yaml", "mod", "conf"];

                        if !BLACK_LIST.contains(&ext)
                            && !dumb_analyzer::is_comment(latest_line, ext)
                        {
                            match context_tag_with_timeout(path.to_path_buf(), lnum).await {
                                Some(tag) if tag.line < start => {
                                    context_lines.reserve_exact(3);

                                    let border_line = "─".repeat(if global().is_nvim {
                                        container_width
                                    } else {
                                        // Vim has a different border width.
                                        container_width - 2
                                    });

                                    context_lines.push(border_line.clone());

                                    // Truncate the right of pattern, 2 whitespaces + 💡
                                    let max_pattern_len = container_width - 4;
                                    let pattern = crate::tools::ctags::trim_pattern(&tag.pattern);
                                    let (mut context_line, to_push) = if pattern.len()
                                        > max_pattern_len
                                    {
                                        // Use the chars instead of indexing the str to avoid the char boundary error.
                                        let p: String =
                                            pattern.chars().take(max_pattern_len - 4 - 2).collect();
                                        (p, "..  💡")
                                    } else {
                                        (String::from(pattern), "  💡")
                                    };
                                    context_line.reserve(to_push.len());
                                    context_line.push_str(to_push);
                                    context_lines.push(context_line);

                                    context_lines.push(border_line);
                                }
                                _ => {}
                            }
                        }
                    }
                }

                let highlight_lnum = highlight_lnum + context_lines.len();

                let header_line = truncated_preview_header();
                let lines = std::iter::once(header_line)
                    .chain(context_lines.into_iter())
                    .chain(self.truncate_preview_lines(lines.into_iter()))
                    .collect::<Vec<_>>();

                tracing::debug!(
                    provider_id = %self.context.provider_id,
                    lines_len = lines.len(),
                    "<== message(out) preview file content",
                );

                if let Some(syntax) = crate::stdio_server::vim::syntax_for(path) {
                    json!({ "lines": lines, "syntax": syntax, "hi_lnum": highlight_lnum })
                } else {
                    json!({ "lines": lines, "fname": fname, "hi_lnum": highlight_lnum })
                }
            }
            Err(err) => {
                tracing::error!(
                    ?path,
                    provider_id = %self.context.provider_id,
                    ?err,
                    "Couldn't read first lines",
                );
                let header_line = truncated_preview_header();
                let lines = vec![
                    header_line,
                    format!("Error while previewing the file: {err}"),
                ];
                json!({ "lines": lines, "fname": fname })
            }
        }
    }

    fn try_refresh_cache(&self, latest_line: &str) {
        if self.context.provider_id.as_str() == "grep2" {
            if let Some(ref cache_line) = self.cache_line {
                if cache_line != latest_line {
                    tracing::debug!(?latest_line, ?cache_line, "The cache is probably outdated");
                    let dir = self.context.cwd.clone();

                    let shell_cmd = crate::command::grep::rg_shell_command(&dir);
                    let job_id = utility::calculate_hash(&shell_cmd);

                    if job::reserve(job_id) {
                        tokio::task::spawn_blocking(move || {
                            tracing::debug!(?dir, "Refreshing grep2 cache");
                            match crate::command::grep::refresh_cache(dir) {
                                Ok(total) => {
                                    tracing::debug!(total, "Refresh the grep2 cache successfully");
                                }
                                Err(e) => {
                                    tracing::error!(error = ?e, "Failed to refresh grep2 cache")
                                }
                            }
                            job::unreserve(job_id);
                        });
                    } else {
                        tracing::debug!(
                            ?dir,
                            "There is already a grep2 job running, skip freshing the cache"
                        );
                    }
                }
            }
        }
    }

    /// Truncates the lines that are awfully long as vim might have some performence issue with
    /// them.
    ///
    /// Ref https://github.com/liuchengxu/vim-clap/issues/543
    fn truncate_preview_lines(
        &self,
        lines: impl Iterator<Item = String>,
    ) -> impl Iterator<Item = String> {
        previewer::truncate_preview_lines(self.max_line_width(), lines)
    }

    /// Returns the maximum line width.
    #[inline]
    fn max_line_width(&self) -> usize {
        2 * self.context.display_winwidth as usize
    }
}

async fn context_tag_with_timeout(path: PathBuf, lnum: usize) -> Option<BufferTag> {
    const TIMEOUT: Duration = Duration::from_millis(300);

    match tokio::time::timeout(TIMEOUT, current_context_tag_async(path.as_path(), lnum)).await {
        Ok(res) => res,
        Err(_) => {
            tracing::debug!(timeout = ?TIMEOUT, "⏳ Did not get the context tag in time");
            None
        }
    }
}
