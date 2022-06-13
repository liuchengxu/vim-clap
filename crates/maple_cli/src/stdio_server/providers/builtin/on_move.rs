use std::borrow::Cow;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use once_cell::sync::Lazy;
use serde_json::json;

use pattern::*;
use types::PreviewInfo;

use crate::command::ctags::buffer_tags::{
    current_context_tag, current_context_tag_async, BufferTagInfo,
};
use crate::previewer::{self, vim_help::HelpTagPreview};
use crate::stdio_server::{
    global, providers::filer, session::SessionContext, write_response, MethodCall,
};
use crate::utils::build_abs_path;

static IS_FERESHING_CACHE: Lazy<AtomicBool> = Lazy::new(|| AtomicBool::new(false));

/// We want to preview a line of a file.
#[derive(Debug, Clone)]
pub struct Position {
    pub path: PathBuf,
    pub lnum: usize,
}

impl Position {
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
    Grep(Position),
    BLines(Position),
    ProjTags(Position),
    BufferTags(Position),
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
            "filer" => unreachable!("filer has been handled beforehand"),
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
                Self::ProjTags(Position::new(path, lnum))
            }
            "coc_location" | "grep" | "grep2" => {
                let mut try_extract_file_path = |line: &str| {
                    let (fpath, lnum, _col, cache_line) =
                        extract_grep_position(line).context("Couldn't extract grep position")?;

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

                let (path, lnum) = try_extract_file_path(&curline)?;

                Self::Grep(Position::new(path, lnum))
            }
            "dumb_jump" => {
                let (_def_kind, fpath, lnum, _col) =
                    extract_jump_line_info(&curline).context("Couldn't extract jump line info")?;
                let mut path: PathBuf = context.cwd.clone();
                path.push(&fpath);
                Self::Grep(Position::new(path, lnum))
            }
            "blines" => {
                let lnum = extract_blines_lnum(&curline).context("Couldn't extract buffer lnum")?;
                let path = context.start_buffer_path.clone();
                Self::BLines(Position::new(path, lnum))
            }
            "tags" => {
                let lnum =
                    extract_buf_tags_lnum(&curline).context("Couldn't extract buffer tags")?;
                let path = context.start_buffer_path.clone();
                Self::BufferTags(Position::new(path, lnum))
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
    /// When the line extracted from the cache mismatches the latest
    /// preview line content, which means the cache is outdated, we
    /// should refresh the cache.
    ///
    /// Currently only for the provider `grep2`.
    pub cache_line: Option<String>,
}

impl<'a> OnMoveHandler<'a> {
    pub fn create(
        msg: &MethodCall,
        context: &'a SessionContext,
        curline: Option<String>,
    ) -> Result<Self> {
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
                cache_line: None,
            });
        }
        let (inner, cache_line) = OnMove::new(curline, context)?;
        Ok(Self {
            msg_id,
            size: context.sensible_preview_size(),
            context,
            inner,
            cache_line,
        })
    }

    pub async fn handle(&self) -> Result<()> {
        use OnMove::*;
        match &self.inner {
            BLines(position) | Grep(position) | ProjTags(position) | BufferTags(position) => {
                self.preview_file_at(position).await
            }
            Filer(path) if path.is_dir() => self.preview_directory(&path)?,
            Files(path) | Filer(path) | History(path) => self.preview_file(&path)?,
            Commit(rev) => self.show_commit(rev)?,
            HelpTags {
                subject,
                doc_filename,
                runtimepath,
            } => self.preview_help_subject(subject, doc_filename, runtimepath),
        }

        Ok(())
    }

    fn send_response(&self, result: serde_json::value::Value) {
        let provider_id = &self.context.provider_id;
        write_response(json!({ "id": self.msg_id, "provider_id": provider_id, "result": result }));
    }

    fn show_commit(&self, rev: &str) -> Result<()> {
        let stdout = self.context.execute(&format!("git show {}", rev))?;
        let stdout_str = String::from_utf8_lossy(&stdout);
        let lines = stdout_str
            .split('\n')
            .take(self.size * 2)
            .collect::<Vec<_>>();
        self.send_response(json!({ "lines": lines }));
        Ok(())
    }

    fn preview_help_subject(&self, subject: &str, doc_filename: &str, runtimepath: &str) {
        let preview_tag = HelpTagPreview::new(subject, doc_filename, runtimepath);
        if let Some((fname, lines)) = preview_tag.get_help_lines(self.size * 2) {
            let lines = std::iter::once(fname.clone())
                .chain(lines.into_iter())
                .collect::<Vec<_>>();
            self.send_response(
                json!({ "syntax": "help", "lines": lines, "hi_lnum": 1, "fname": fname }),
            );
        } else {
            tracing::debug!(?preview_tag, "Can not find the preview help lines");
        }
    }

    fn try_refresh_cache(&self, latest_line: &str) {
        if IS_FERESHING_CACHE.load(Ordering::SeqCst) {
            tracing::debug!(
                "Skipping the cache refreshing as there is already one that is running or waitting"
            );
            return;
        }
        if self.context.provider_id.as_str() == "grep2" {
            if let Some(ref cache_line) = self.cache_line {
                if cache_line != latest_line {
                    tracing::debug!(?latest_line, ?cache_line, "The cache is probably outdated");
                    let dir = self.context.cwd.clone();
                    IS_FERESHING_CACHE.store(true, Ordering::SeqCst);
                    // Spawn a future in the background
                    tokio::task::spawn_blocking(|| {
                        tracing::debug!(?dir, "Attempting to refresh grep2 cache");
                        match crate::command::grep::refresh_cache(dir) {
                            Ok(total) => {
                                tracing::debug!(total, "Refresh the grep2 cache successfully");
                            }
                            Err(e) => {
                                tracing::error!(error = ?e, "Failed to refresh the grep2 cache")
                            }
                        }
                        IS_FERESHING_CACHE.store(false, Ordering::SeqCst);
                    });
                }
            }
        }
    }

    async fn preview_file_at(&self, position: &Position) {
        tracing::debug!(?position, "Previewing file");

        let Position { path, lnum } = position;

        match utility::read_preview_lines(path, *lnum, self.size) {
            Ok(PreviewInfo {
                lines,
                highlight_lnum,
                start,
                ..
            }) => {
                let container_width = self.context.display_winwidth as usize;

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
                            match context_tag_with_timeout(path.to_path_buf(), *lnum).await {
                                Some(tag) if tag.line < start => {
                                    context_lines.reserve_exact(3);

                                    let border_line =
                                        "─".repeat(if crate::stdio_server::global().is_nvim {
                                            container_width
                                        } else {
                                            // Vim has a different border width.
                                            container_width - 2
                                        });

                                    context_lines.push(border_line.clone());

                                    // Truncate the right of pattern, 2 whitespaces + 💡
                                    let max_pattern_len = container_width - 4;
                                    let pattern = tag.extract_pattern();
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

                let fname = path.display().to_string();
                let max_fname_len = container_width - 1 - crate::utils::display_width(*lnum);
                let header_line = match self.inner {
                    OnMove::Grep(_) if !crate::stdio_server::global().is_nvim => {
                        // cwd is shown via the popup title, no need to include it again.
                        let cwd_relative =
                            fname.replacen(self.context.cwd.to_str().unwrap(), ".", 1);
                        format!("{}:{}", cwd_relative, lnum)
                    }
                    _ => format!("{}:{}", truncate_absolute_path(&fname, max_fname_len), lnum),
                };

                let lines = std::iter::once(header_line)
                    .chain(context_lines.into_iter())
                    .chain(self.truncate_preview_lines(lines.into_iter()))
                    .collect::<Vec<_>>();

                tracing::debug!(
                    msg_id = self.msg_id,
                    provider_id = %self.context.provider_id,
                    lines_len = lines.len(),
                    "<== message(out) preview file content",
                );

                if let Some(syntax) = crate::stdio_server::vim::syntax_for(path) {
                    self.send_response(
                        json!({ "lines": lines, "syntax": syntax, "hi_lnum": highlight_lnum }),
                    );
                } else {
                    self.send_response(
                        json!({ "lines": lines, "fname": fname, "hi_lnum": highlight_lnum }),
                    );
                }
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

    /// Truncates the lines that are awfully long as vim might have some performence issue with
    /// them.
    ///
    /// Ref https://github.com/liuchengxu/vim-clap/issues/543
    fn truncate_preview_lines(
        &self,
        lines: impl Iterator<Item = String>,
    ) -> impl Iterator<Item = String> {
        previewer::truncate_preview_lines(self.max_width(), lines)
    }

    /// Returns the maximum line width.
    #[inline]
    fn max_width(&self) -> usize {
        2 * self.context.display_winwidth as usize
    }

    fn preview_file<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let (lines, fname) =
            previewer::preview_file(path.as_ref(), 2 * self.size, self.max_width())?;
        if let Some(syntax) = crate::stdio_server::vim::syntax_for(path.as_ref()) {
            self.send_response(json!({ "lines": lines, "syntax": syntax }));
        } else {
            self.send_response(json!({ "lines": lines, "fname": fname }));
        }
        Ok(())
    }

    fn preview_directory<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let enable_icon = global().enable_icon;
        let lines = filer::read_dir_entries(&path, enable_icon, Some(2 * self.size))?;
        self.send_response(json!({ "lines": lines, "is_dir": true }));
        Ok(())
    }
}

async fn context_tag_with_timeout(path: PathBuf, lnum: usize) -> Option<BufferTagInfo> {
    const TIMEOUT: Duration = Duration::from_millis(300);

    match tokio::time::timeout(TIMEOUT, async move {
        current_context_tag_async(path.as_path(), lnum).await
    })
    .await
    {
        Ok(res) => res,
        Err(_) => {
            tracing::debug!(timeout = ?TIMEOUT, "⏳ Did not get the context tag in time");
            None
        }
    }
}

// /home/xlc/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/lib/rustlib/src/rust/library/alloc/src/string.rs
fn truncate_absolute_path(abs_path: &str, max_len: usize) -> Cow<'_, str> {
    if abs_path.len() > max_len {
        let gap = abs_path.len() - max_len;

        if let Some(home_dir) = crate::utils::HOME_DIR.as_path().to_str() {
            // ~/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/lib/rustlib/src/rust/library/alloc/src/string.rs
            if home_dir.len() > gap {
                return abs_path.replacen(home_dir, "~", 1).into();
            }

            let sep = std::path::MAIN_SEPARATOR;

            // ~/.rustup/.../github.com/paritytech/substrate/frame/system/src/lib.rs
            let home_stripped = &abs_path.trim_start_matches(home_dir)[1..];
            if let Some((first, target)) = home_stripped.split_once(sep) {
                let mut hidden = 0usize;
                for component in target.split(sep) {
                    if hidden > gap + 2 {
                        let mut target = target.to_string();
                        target.replace_range(..hidden - 1, "...");
                        return format!("~{sep}{first}{sep}{target}").into();
                    } else {
                        hidden += component.len() + 1;
                    }
                }
            }
        } else {
            // Truncate the left of absolute path string.
            // ../stable-x86_64-unknown-linux-gnu/lib/rustlib/src/rust/library/alloc/src/string.rs
            if let Some((offset, _)) = abs_path.char_indices().nth(abs_path.len() - max_len + 2) {
                let mut abs_path = abs_path.to_string();
                abs_path.replace_range(..offset, "..");
                return abs_path.into();
            }
        }
    }

    abs_path.into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_truncate_absolute_path() {
        #[cfg(not(target_os = "windows"))]
        let p = ".rustup/toolchains/stable-x86_64-unknown-linux-gnu/lib/rustlib/src/rust/library/alloc/src/string.rs";
        #[cfg(target_os = "windows")]
        let p = r#".rustup\toolchains\stable-x86_64-unknown-linux-gnu\lib\rustlib\src\rust\library\alloc\src\string.rs"#;
        let abs_path = format!(
            "{}{}{}",
            crate::utils::HOME_DIR.as_path().to_str().unwrap(),
            std::path::MAIN_SEPARATOR,
            p
        );
        let max_len = 60;
        #[cfg(not(target_os = "windows"))]
        let expected = "~/.rustup/.../src/rust/library/alloc/src/string.rs";
        #[cfg(target_os = "windows")]
        let expected = r#"~\.rustup\...\src\rust\library\alloc\src\string.rs"#;
        assert_eq!(truncate_absolute_path(&abs_path, max_len), expected)
    }
}
