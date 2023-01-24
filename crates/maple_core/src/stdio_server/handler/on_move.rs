use crate::paths::truncate_absolute_path;
use crate::previewer;
use crate::previewer::vim_help::HelpTagPreview;
use crate::previewer::{get_file_preview, FilePreview};
use crate::stdio_server::job;
use crate::stdio_server::provider::{read_dir_entries, Context, ProviderSource};
use crate::stdio_server::vim::preview_syntax;
use crate::tools::ctags::{current_context_tag_async, BufferTag};
use anyhow::{anyhow, Result};
use pattern::*;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::time::Duration;
use utils::display_width;

/// Preview content.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Preview {
    pub lines: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub syntax: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fname: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hi_lnum: Option<usize>,
}

impl Preview {
    pub fn new(lines: Vec<String>) -> Self {
        Self {
            lines,
            ..Default::default()
        }
    }
}

#[derive(Debug, Hash, Eq, PartialEq, Clone)]
pub enum PreviewTarget {
    /// List the entries under a directory.
    Directory(PathBuf),
    /// Start from the beginning of a file.
    File(PathBuf),
    /// A specific location in a file.
    LineInFile { path: PathBuf, line_number: usize },
    /// Commit revision.
    Commit(String),
    /// For the provider `help_tags`.
    HelpTags {
        subject: String,
        doc_filename: String,
        runtimepath: String,
    },
}

impl PreviewTarget {
    pub fn path(&self) -> Option<&Path> {
        match self {
            Self::File(path) | Self::Directory(path) | Self::LineInFile { path, .. } => Some(path),
            _ => None,
        }
    }
}

fn parse_preview_target(curline: String, ctx: &Context) -> Result<(PreviewTarget, Option<String>)> {
    let err = || {
        anyhow!(
            "Failed to parse PreviewTarget for provider_id: {} from `{curline}`",
            ctx.provider_id()
        )
    };

    // Store the line context we see in the search result, but it may be out-dated due to the
    // cache is being used, especially for the providers like grep which potentially have tons of
    // items.
    //
    // If the line we see mismatches the actual line content in the preview, the content in which
    // is always accurate, try to refresh the cache and reload.
    let mut line_content = None;

    let preview_target = match ctx.provider_id() {
        "files" | "git_files" => PreviewTarget::File(ctx.cwd.join(&curline)),
        "recent_files" => PreviewTarget::File(PathBuf::from(&curline)),
        "history" => {
            let path = if curline.starts_with('~') {
                crate::paths::expand_tilde(curline)
            } else {
                ctx.cwd.join(&curline)
            };
            PreviewTarget::File(path)
        }
        "coc_location" | "grep" | "live_grep" => {
            let mut try_extract_file_path = |line: &str| {
                let (fpath, lnum, _col, cache_line) =
                    extract_grep_position(line).ok_or_else(err)?;

                line_content.replace(cache_line.into());

                let fpath = fpath.strip_prefix("./").unwrap_or(fpath);
                let path = ctx.cwd.join(fpath);

                Ok::<_, anyhow::Error>((path, lnum))
            };

            let (path, line_number) = try_extract_file_path(&curline)?;

            PreviewTarget::LineInFile { path, line_number }
        }
        "dumb_jump" => {
            let (_def_kind, fpath, line_number, _col) =
                extract_jump_line_info(&curline).ok_or_else(err)?;
            let path = ctx.cwd.join(fpath);
            PreviewTarget::LineInFile { path, line_number }
        }
        "blines" => {
            let line_number = extract_blines_lnum(&curline).ok_or_else(err)?;
            let path = ctx.env.start_buffer_path.clone();
            PreviewTarget::LineInFile { path, line_number }
        }
        "tags" => {
            let line_number = extract_buf_tags_lnum(&curline).ok_or_else(err)?;
            let path = ctx.env.start_buffer_path.clone();
            PreviewTarget::LineInFile { path, line_number }
        }
        "proj_tags" => {
            let (line_number, p) = extract_proj_tags(&curline).ok_or_else(err)?;
            let path = ctx.cwd.join(p);
            PreviewTarget::LineInFile { path, line_number }
        }
        "commits" | "bcommits" => {
            let rev = extract_commit_rev(&curline).ok_or_else(err)?;
            PreviewTarget::Commit(rev.into())
        }
        unknown_provider_id => {
            return Err(anyhow!(
                "Failed to parse PreviewTarget, you probably forget to \
                    add an implementation for this provider: {unknown_provider_id}",
            ))
        }
    };

    Ok((preview_target, line_content))
}

/// Returns `true` if the file path of preview file should be truncateted relative to cwd.
fn should_truncate_cwd_relative(provider_id: &str) -> bool {
    const SET: &[&str] = &[
        "files",
        "git_files",
        "grep",
        "live_grep",
        "coc_location",
        "proj_tags",
    ];
    SET.contains(&provider_id)
}

#[derive(Debug)]
pub struct CachedPreviewImpl<'a> {
    pub ctx: &'a Context,
    pub preview_height: usize,
    pub preview_target: PreviewTarget,
    /// When the line extracted from the cache mismatches the latest
    /// preview line content, which means the cache is outdated, we
    /// should refresh the cache.
    ///
    /// Currently only for the provider `grep`.
    pub cache_line: Option<String>,
}

impl<'a> CachedPreviewImpl<'a> {
    pub fn new(curline: String, preview_height: usize, ctx: &'a Context) -> Result<Self> {
        let (preview_target, cache_line) = parse_preview_target(curline, ctx)?;

        Ok(Self {
            ctx,
            preview_height,
            preview_target,
            cache_line,
        })
    }

    pub async fn get_preview(&self) -> Result<Preview> {
        if let Some(preview) = self.ctx.cached_preview(&self.preview_target) {
            return Ok(preview);
        }

        let preview = match &self.preview_target {
            PreviewTarget::Directory(path) => self.preview_directory(path)?,
            PreviewTarget::File(path) => self.preview_file(path)?,
            PreviewTarget::LineInFile { path, line_number } => {
                self.preview_file_at(path, *line_number).await
            }
            PreviewTarget::Commit(rev) => self.preview_commits(rev)?,
            PreviewTarget::HelpTags {
                subject,
                doc_filename,
                runtimepath,
            } => self.preview_help_subject(subject, doc_filename, runtimepath),
        };

        self.ctx
            .insert_preview(self.preview_target.clone(), preview.clone());

        Ok(preview)
    }

    fn preview_commits(&self, rev: &str) -> std::io::Result<Preview> {
        let stdout = self.ctx.exec_cmd(&format!("git show {rev}"))?;
        let stdout_str = String::from_utf8_lossy(&stdout);
        let lines = stdout_str
            .split('\n')
            .take(self.preview_height)
            .map(Into::into)
            .collect::<Vec<_>>();
        Ok(Preview::new(lines))
    }

    fn preview_help_subject(
        &self,
        subject: &str,
        doc_filename: &str,
        runtimepath: &str,
    ) -> Preview {
        let preview_tag = HelpTagPreview::new(subject, doc_filename, runtimepath);
        if let Some((fname, lines)) = preview_tag.get_help_lines(self.preview_height) {
            let lines = std::iter::once(fname.clone())
                .chain(lines.into_iter())
                .collect::<Vec<_>>();
            Preview {
                lines,
                hi_lnum: Some(1),
                fname: Some(fname),
                syntax: Some("help".into()),
            }
        } else {
            tracing::debug!(?preview_tag, "Can not find the preview help lines");
            Preview::new(vec!["Can not find the preview help lines".into()])
        }
    }

    fn preview_directory<P: AsRef<Path>>(&self, path: P) -> std::io::Result<Preview> {
        let enable_icon = self.ctx.env.icon.enabled();
        let lines = read_dir_entries(&path, enable_icon, Some(self.preview_height))?;
        let mut lines = if lines.is_empty() {
            vec!["Directory is empty".to_string()]
        } else {
            lines
        };

        let mut title = path.as_ref().display().to_string();
        if title.ends_with(std::path::MAIN_SEPARATOR) {
            title.pop();
        }
        title.push(':');
        lines.insert(0, title);

        Ok(Preview::new(lines))
    }

    fn preview_file<P: AsRef<Path>>(&self, path: P) -> std::io::Result<Preview> {
        let path = path.as_ref();

        if !path.is_file() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Failed to preview as {} is not a file", path.display()),
            ));
        }

        let handle_io_error = |e: &std::io::Error| {
            if e.kind() == std::io::ErrorKind::NotFound {
                tracing::debug!(
                    "TODO: {} not found, the files cache might be invalid, try refreshing the cache",
                    path.display()
                );
            }
        };

        let (lines, fname) = if !self.ctx.env.is_nvim {
            let (lines, abs_path) =
                previewer::preview_file(path, self.preview_height, self.max_line_width()).map_err(
                    |e| {
                        handle_io_error(&e);
                        e
                    },
                )?;
            // cwd is shown via the popup title, no need to include it again.
            let cwd_relative = abs_path.replacen(self.ctx.cwd.as_str(), ".", 1);
            let mut lines = lines;
            lines[0] = cwd_relative;
            (lines, abs_path)
        } else {
            let max_fname_len = self.ctx.env.display_winwidth - 1;
            previewer::preview_file_with_truncated_title(
                path,
                self.preview_height,
                self.max_line_width(),
                max_fname_len,
            )
            .map_err(|e| {
                handle_io_error(&e);
                e
            })?
        };

        if std::fs::metadata(path)?.len() == 0 {
            let mut lines = lines;
            lines.push("<Empty file>".to_string());
            Ok(Preview {
                lines,
                fname: Some(fname),
                ..Default::default()
            })
        } else if let Some(syntax) = preview_syntax(path) {
            Ok(Preview {
                lines,
                syntax: Some(syntax.into()),
                ..Default::default()
            })
        } else {
            Ok(Preview {
                lines,
                fname: Some(fname),
                ..Default::default()
            })
        }
    }

    async fn preview_file_at(&self, path: &Path, lnum: usize) -> Preview {
        tracing::debug!(path = ?path.display(), lnum, "Previewing file");

        let container_width = self.ctx.env.display_winwidth;
        let fname = path.display().to_string();

        let truncated_preview_header = || {
            if !self.ctx.env.is_nvim && should_truncate_cwd_relative(self.ctx.provider_id()) {
                // cwd is shown via the popup title, no need to include it again.
                let cwd_relative = fname.replacen(self.ctx.cwd.as_str(), ".", 1);
                format!("{cwd_relative}:{lnum}")
            } else {
                let max_fname_len = container_width - 1 - display_width(lnum);
                let truncated_abs_path = truncate_absolute_path(&fname, max_fname_len);
                format!("{truncated_abs_path}:{lnum}")
            }
        };

        match get_file_preview(path, lnum, self.preview_height) {
            Ok(FilePreview {
                lines,
                highlight_lnum,
                start,
                ..
            }) => {
                let mut context_lines = Vec::new();

                // Some checks against the latest preview line.
                if let Some(latest_line) = lines.get(highlight_lnum - 1) {
                    // TODO: No long needed once switched to libgrep officically.
                    // self.try_refresh_cache(latest_line);

                    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                        const BLACK_LIST: &[&str] =
                            &["log", "txt", "lock", "toml", "yaml", "mod", "conf"];

                        if !BLACK_LIST.contains(&ext)
                            && !dumb_analyzer::is_comment(latest_line, ext)
                        {
                            match context_tag_with_timeout(path, lnum).await {
                                Some(tag) if tag.line < start => {
                                    context_lines.reserve_exact(3);

                                    let border_line = "─".repeat(if self.ctx.env.is_nvim {
                                        container_width
                                    } else {
                                        // Vim has a different border width.
                                        container_width - 2
                                    });

                                    context_lines.push(border_line.clone());

                                    // Truncate the right of pattern, 2 whitespaces + 💡
                                    let max_pattern_len = container_width - 4;
                                    let pattern = tag.trimmed_pattern();
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

                if let Some(syntax) = preview_syntax(path) {
                    Preview {
                        lines,
                        syntax: Some(syntax.into()),
                        hi_lnum: Some(highlight_lnum),
                        fname: None,
                    }
                } else {
                    Preview {
                        lines,
                        syntax: None,
                        hi_lnum: Some(highlight_lnum),
                        fname: Some(fname),
                    }
                }
            }
            Err(err) => {
                tracing::error!(
                    ?path,
                    provider_id = %self.ctx.provider_id(),
                    ?err,
                    "Couldn't read first lines",
                );
                let header_line = truncated_preview_header();
                let lines = vec![
                    header_line,
                    format!("Error while previewing the file: {err}"),
                ];
                Preview {
                    lines,
                    fname: Some(fname),
                    ..Default::default()
                }
            }
        }
    }

    // TODO: Only run for these provider using custom shell command.
    #[allow(unused)]
    fn try_refresh_cache(&self, latest_line: &str) {
        if self.ctx.provider_id() == "grep" {
            if let Some(ref cache_line) = self.cache_line {
                if cache_line != latest_line {
                    tracing::debug!(?latest_line, ?cache_line, "The cache is probably outdated");

                    let shell_cmd = crate::tools::rg::rg_shell_command(&self.ctx.cwd);
                    let job_id = utils::calculate_hash(&shell_cmd);

                    if job::reserve(job_id) {
                        let ctx = self.ctx.clone();

                        // TODO: Refresh with a timeout.
                        tokio::task::spawn_blocking(move || {
                            tracing::debug!(cwd = ?ctx.cwd, "Refreshing grep cache");
                            let new_digest = match crate::tools::rg::refresh_cache(&ctx.cwd) {
                                Ok(digest) => {
                                    tracing::debug!(
                                        total = digest.total,
                                        "Refresh the grep cache successfully"
                                    );
                                    digest
                                }
                                Err(e) => {
                                    tracing::error!(error = ?e, "Failed to refresh grep cache");
                                    return;
                                }
                            };
                            let new = ProviderSource::CachedFile {
                                total: new_digest.total,
                                path: new_digest.cached_path,
                                refreshed: true,
                            };
                            ctx.set_provider_source(new);
                            job::unreserve(job_id);

                            if !ctx.terminated.load(Ordering::SeqCst) {
                                let _ = ctx.vim.echo_info("Out-dated cache refreshed");
                            }
                        });
                    } else {
                        tracing::debug!(
                            cwd = ?self.ctx.cwd,
                            "There is already a grep job running, skip freshing the cache"
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
        previewer::truncate_lines(lines, self.max_line_width())
    }

    /// Returns the maximum line width.
    #[inline]
    fn max_line_width(&self) -> usize {
        2 * self.ctx.env.display_winwidth
    }
}

async fn context_tag_with_timeout(path: &Path, lnum: usize) -> Option<BufferTag> {
    const TIMEOUT: Duration = Duration::from_millis(300);

    match tokio::time::timeout(TIMEOUT, current_context_tag_async(path, lnum)).await {
        Ok(res) => res,
        Err(_) => {
            tracing::debug!(timeout = ?TIMEOUT, "⏳ Did not get the context tag in time");
            None
        }
    }
}
