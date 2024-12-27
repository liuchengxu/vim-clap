use crate::previewer;
use crate::previewer::text_file::{generate_text_preview, TextPreview};
use crate::previewer::vim_help::HelpTagPreview;
use crate::stdio_server::job;
use crate::stdio_server::plugin::syntax::convert_raw_ts_highlights_to_vim_highlights;
use crate::stdio_server::plugin::syntax::sublime::{
    sublime_syntax_by_extension, sublime_syntax_highlight, sublime_theme_exists,
};
use crate::stdio_server::provider::{read_dir_entries, Context, ProviderSource};
use crate::stdio_server::vim::{preview_syntax, VimResult};
use crate::tools::ctags::{current_context_tag, BufferTag};
use maple_config::HighlightEngine;
use paths::{expand_tilde, truncate_absolute_path};
use pattern::*;
use serde::{Deserialize, Serialize};
use std::io::{Error, ErrorKind, Read, Result};
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::time::Duration;
use sublime_syntax::TokenHighlight;
use tokio::sync::oneshot;
use utils::display_width;
use utils::io::SizeChecker;

type SublimeHighlightData = Vec<(usize, Vec<TokenHighlight>)>;

/// (start, length, highlight_group)
type LineHighlightData = Vec<(usize, usize, String)>;

/// (line_number, line_highlights)
type TreeSitterHighlightData = Vec<(usize, LineHighlightData)>;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct VimSyntaxInfo {
    syntax: String,
    fname: String,
}

impl VimSyntaxInfo {
    fn syntax(syntax: String) -> Self {
        Self {
            syntax,
            ..Default::default()
        }
    }

    fn fname(fname: String) -> Self {
        Self {
            fname,
            ..Default::default()
        }
    }

    fn is_empty(&self) -> bool {
        self.syntax.is_empty() && self.fname.is_empty()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HighlightLine {
    /// Number of line (1-based) highlighted in the preview window.
    pub line_number: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub column_range: Option<Range<usize>>,
}

/// Preview content.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Preview {
    pub lines: Vec<String>,

    /// This field is used to tell vim what syntax value
    /// should be used for the highlighting when neither
    /// sublime-syntax nor tree-sitter is available.
    ///
    /// Ideally `syntax` is returned directly, otherwise
    /// `fname` is returned and then Vim will interpret
    /// the syntax value from `fname` on its own.
    #[serde(skip_serializing_if = "VimSyntaxInfo::is_empty")]
    pub vim_syntax_info: VimSyntaxInfo,

    /// Highlights from sublime-syntax highlight engine.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub sublime_syntax_highlights: SublimeHighlightData,

    /// Highlights from tree-sitter highlight engine.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tree_sitter_highlights: TreeSitterHighlightData,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub highlight_line: Option<HighlightLine>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub scrollbar: Option<(usize, usize)>,
}

impl Preview {
    pub fn new(lines: Vec<String>) -> Self {
        Self {
            lines,
            ..Default::default()
        }
    }

    fn new_file_preview(
        lines: Vec<String>,
        scrollbar: Option<(usize, usize)>,
        vim_syntax_info: VimSyntaxInfo,
    ) -> Self {
        Self {
            lines,
            vim_syntax_info,
            scrollbar,
            ..Default::default()
        }
    }

    fn binary_file_preview(path: impl AsRef<Path>) -> Self {
        Self::new_file_preview(
            vec!["<Binary file>".to_string()],
            None,
            VimSyntaxInfo::fname(path.as_ref().display().to_string()),
        )
    }

    fn large_file_preview(size: u64, path: impl AsRef<Path>) -> Self {
        let size_in_gib = size as f64 / (1024.0 * 1024.0 * 1024.0);
        Self::new_file_preview(
            vec![format!(
                "File too large to preview (size: {size_in_gib:.2} GiB)."
            )],
            None,
            VimSyntaxInfo::fname(path.as_ref().display().to_string()),
        )
    }

    fn set_highlights(&mut self, highlight_source: HighlightSource, path: &Path) {
        match highlight_source {
            HighlightSource::Sublime(v) => {
                self.sublime_syntax_highlights = v;
            }
            HighlightSource::TreeSitter(v) => {
                self.tree_sitter_highlights = v;
            }
            HighlightSource::None => {
                if let Some(syntax) = preview_syntax(path) {
                    self.vim_syntax_info.syntax = syntax.into();
                } else {
                    self.vim_syntax_info.fname = path.display().to_string();
                }
            }
        }
    }
}

/// Represents various targets for previews in clap provider.
#[derive(Debug, Hash, Eq, PartialEq, Clone)]
pub enum PreviewTarget {
    /// List the entries under a directory.
    Directory(PathBuf),
    /// Start from the beginning of a file.
    StartOfFile(PathBuf),
    /// Represents a specific location in a file identified by its path and line number.
    LocationInFile {
        path: PathBuf,
        line_number: usize,
        column_range: Option<Range<usize>>,
    },
    /// Represents a Git commit revision specified by its commit hash.
    GitCommit(String),
    /// Specifically for the `help_tags` provider.
    HelpTags {
        subject: String,
        doc_filename: String,
        runtimepath: String,
    },
}

impl PreviewTarget {
    pub fn location_in_file(path: PathBuf, line_number: usize) -> Self {
        Self::LocationInFile {
            path,
            line_number,
            column_range: None,
        }
    }
}

impl PreviewTarget {
    /// Returns the path associated with the enum variant, or `None` if no path exists.
    pub fn path(&self) -> Option<&Path> {
        match self {
            Self::Directory(path) | Self::StartOfFile(path) | Self::LocationInFile { path, .. } => {
                Some(path)
            }
            _ => None,
        }
    }
}

fn parse_preview_target(curline: String, ctx: &Context) -> Result<(PreviewTarget, Option<String>)> {
    let err = || {
        Error::new(
            ErrorKind::Other,
            format!(
                "Failed to parse PreviewTarget for provider_id: {} from `{curline}`",
                ctx.provider_id()
            ),
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
        "files" | "git_files" => PreviewTarget::StartOfFile(ctx.cwd.join(&curline)),
        "recent_files" => PreviewTarget::StartOfFile(PathBuf::from(&curline)),
        "history" => {
            let path = if curline.starts_with('~') {
                expand_tilde(curline)
            } else {
                ctx.cwd.join(&curline)
            };
            PreviewTarget::StartOfFile(path)
        }
        "coc_location" | "grep" | "live_grep" | "igrep" => {
            let mut try_extract_file_path = |line: &str| {
                let (fpath, lnum, _col, cache_line) =
                    extract_grep_position(line).ok_or_else(err)?;

                line_content.replace(cache_line.into());

                let fpath = fpath.strip_prefix("./").unwrap_or(fpath);
                let path = ctx.cwd.join(fpath);

                Ok::<_, Error>((path, lnum))
            };

            let (path, line_number) = try_extract_file_path(&curline)?;

            PreviewTarget::location_in_file(path, line_number)
        }
        "dumb_jump" => {
            let (_def_kind, fpath, line_number, _col) =
                extract_jump_line_info(&curline).ok_or_else(err)?;
            let path = ctx.cwd.join(fpath);
            PreviewTarget::location_in_file(path, line_number)
        }
        "blines" => {
            let line_number = extract_blines_lnum(&curline).ok_or_else(err)?;
            let path = ctx.env.start_buffer_path.clone();
            PreviewTarget::location_in_file(path, line_number)
        }
        "tags" => {
            let line_number = extract_buf_tags_lnum(&curline).ok_or_else(err)?;
            let path = ctx.env.start_buffer_path.clone();
            PreviewTarget::location_in_file(path, line_number)
        }
        "proj_tags" => {
            let (line_number, p) = extract_proj_tags(&curline).ok_or_else(err)?;
            let path = ctx.cwd.join(p);
            PreviewTarget::location_in_file(path, line_number)
        }
        "commits" | "bcommits" => {
            let rev = extract_commit_rev(&curline).ok_or_else(err)?;
            PreviewTarget::GitCommit(rev.into())
        }
        unknown_provider_id => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!(
                    "Failed to parse PreviewTarget, you probably forget to \
                    add an implementation for this provider: {unknown_provider_id}",
                ),
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
        "dumb_jump",
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

    pub fn with_preview_target(
        preview_target: PreviewTarget,
        preview_height: usize,
        ctx: &'a Context,
    ) -> Self {
        Self {
            ctx,
            preview_height,
            preview_target,
            cache_line: None,
        }
    }

    pub async fn get_preview(&self) -> VimResult<(PreviewTarget, Preview)> {
        if let Some(preview) = self
            .ctx
            .preview_manager
            .cached_preview(&self.preview_target)
        {
            return Ok((self.preview_target.clone(), preview));
        }

        let now = std::time::Instant::now();

        let preview = match &self.preview_target {
            PreviewTarget::Directory(path) => self.preview_directory(path)?,
            PreviewTarget::StartOfFile(path) => self.preview_file(path).await?,
            PreviewTarget::LocationInFile {
                path,
                line_number,
                column_range,
            } => {
                let container_width = self.ctx.preview_winwidth().await?;
                self.preview_file_at(path, *line_number, column_range.clone(), container_width)
                    .await
            }
            PreviewTarget::GitCommit(rev) => self.preview_commits(rev)?,
            PreviewTarget::HelpTags {
                subject,
                doc_filename,
                runtimepath,
            } => self.preview_help_subject(subject, doc_filename, runtimepath),
        };

        let elapsed = now.elapsed().as_millis();
        if elapsed > 1000 {
            tracing::warn!(preview_target = ?self.preview_target, "Fetching preview took too long: {elapsed:?} ms");
        }

        self.ctx
            .preview_manager
            .insert_preview(self.preview_target.clone(), preview.clone());

        Ok((self.preview_target.clone(), preview))
    }

    fn preview_commits(&self, rev: &str) -> Result<Preview> {
        let stdout = self.ctx.exec_cmd(&format!("git show {rev}"))?;
        let stdout_str = String::from_utf8_lossy(&stdout);
        let lines = stdout_str
            .split('\n')
            .take(self.preview_height)
            .map(Into::into)
            .collect::<Vec<_>>();
        let mut preview = Preview::new(lines);
        preview.vim_syntax_info.syntax = "diff".to_string();
        Ok(preview)
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
                .chain(lines)
                .collect::<Vec<_>>();
            Preview {
                lines,
                highlight_line: Some(HighlightLine {
                    line_number: 1,
                    ..Default::default()
                }),
                vim_syntax_info: VimSyntaxInfo::syntax("help".into()),
                ..Default::default()
            }
        } else {
            tracing::debug!(?preview_tag, "Can not find the preview help lines");
            Preview::new(vec!["Can not find the preview help lines".into()])
        }
    }

    fn preview_directory<P: AsRef<Path>>(&self, path: P) -> Result<Preview> {
        let enable_icon = self.ctx.env.icon.enabled();
        let lines = read_dir_entries(&path, enable_icon, Some(self.preview_height))?;
        let mut lines = if lines.is_empty() {
            vec!["<Empty directory>".to_string()]
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

    async fn preview_file<P: AsRef<Path>>(&self, path: P) -> Result<Preview> {
        let path = path.as_ref();

        let file_size_tier = match detect_file_class(path)? {
            FileClass::NotRegularFile => {
                return Err(Error::new(
                    ErrorKind::Other,
                    format!("Failed to preview as {} is not a file", path.display()),
                ));
            }
            FileClass::Binary => {
                return Ok(Preview::binary_file_preview(path));
            }
            FileClass::Text(file_size_tier) => {
                if let utils::io::FileSizeTier::Large(size) = file_size_tier {
                    return Ok(Preview::large_file_preview(size, path));
                }
                file_size_tier
            }
        };

        let handle_io_error = |e: &Error| {
            if e.kind() == ErrorKind::NotFound {
                tracing::debug!(
                    "TODO: {} not found, the files cache might be invalid, try refreshing the cache",
                    path.display()
                );
            }
        };

        let (lines, fname) = match (self.ctx.env.is_nvim, self.ctx.env.has_nvim_09) {
            (true, false) => {
                // Title is not available before nvim 0.9
                let max_fname_len = self.ctx.env.display_line_width - 1;
                let previewer::text_file::TextLines {
                    lines,
                    display_path,
                } = previewer::text_file::preview_file(
                    path,
                    self.preview_height,
                    self.max_line_width(),
                    Some(max_fname_len),
                    file_size_tier,
                )
                .inspect_err(handle_io_error)?;
                (lines, display_path)
            }
            _ => {
                let previewer::text_file::TextLines {
                    lines,
                    display_path: abs_path,
                } = previewer::text_file::preview_file(
                    path,
                    self.preview_height,
                    self.max_line_width(),
                    None,
                    file_size_tier,
                )
                .inspect_err(handle_io_error)?;

                // cwd is already shown in the popup title, no need to include it again.
                let cwd_relative = abs_path.replacen(self.ctx.cwd.as_str(), ".", 1);
                let mut lines = lines;
                lines[0] = cwd_relative;

                (lines, abs_path)
            }
        };

        if file_size_tier.is_empty() {
            let mut lines = lines;
            lines.push("<Empty file>".to_string());
            return Ok(Preview::new_file_preview(
                lines,
                None,
                VimSyntaxInfo::fname(fname),
            ));
        }

        let highlight_source = if file_size_tier.is_small() {
            SyntaxHighlighter {
                context: HighlightingContext {
                    lines: lines.clone(),
                    path: path.to_path_buf(),
                    line_number_offset: 0,
                    max_line_width: self.max_line_width(),
                    range: 0..lines.len(),
                    maybe_code_context: None,
                },
                timeout: 200,
            }
            .highlight_with_timeout()
            .await
        } else {
            HighlightSource::None
        };

        // Only display the scrollbar when it's not a large file.
        let scrollbar = if file_size_tier.can_process() {
            let end = lines.len();
            if self.ctx.env.should_add_scrollbar(end) {
                let total = utils::io::line_count(path)?;
                compute_scrollbar_position(self.ctx, 0, end, total)
            } else {
                None
            }
        } else {
            None
        };

        let mut preview = Preview::new_file_preview(lines, scrollbar, VimSyntaxInfo::default());
        preview.set_highlights(highlight_source, path);

        Ok(preview)
    }

    async fn preview_file_at(
        &self,
        path: &Path,
        lnum: usize,
        column_range: Option<Range<usize>>,
        container_width: usize,
    ) -> Preview {
        tracing::debug!(path = %path.display(), "Previewing file at line {lnum}");

        match detect_file_class(path) {
            Ok(FileClass::NotRegularFile) => {
                return Preview::new_file_preview(
                    vec!["<Not a regular file>".to_string()],
                    None,
                    VimSyntaxInfo::fname(path.display().to_string()),
                );
            }
            Ok(FileClass::Binary) => return Preview::binary_file_preview(path),
            Ok(FileClass::Text(file_size_tier)) => {
                if let utils::io::FileSizeTier::Large(size) = file_size_tier {
                    return Preview::large_file_preview(size, path);
                }
            }
            Err(err) => {
                return Preview::new_file_preview(
                    vec![err.to_string()],
                    None,
                    VimSyntaxInfo::fname(path.display().to_string()),
                );
            }
        };

        let fname = path.display().to_string();

        let truncated_preview_header = || {
            let support_float_title = !self.ctx.env.is_nvim || self.ctx.env.has_nvim_09;
            if support_float_title && should_truncate_cwd_relative(self.ctx.provider_id()) {
                // cwd is shown via the popup title, no need to include it again.
                let cwd_relative = fname.replacen(self.ctx.cwd.as_str(), ".", 1);
                format!("{cwd_relative}:{lnum}")
            } else {
                let max_fname_len = container_width - 1 - display_width(lnum);
                let truncated_abs_path = truncate_absolute_path(&fname, max_fname_len);
                format!("{truncated_abs_path}:{lnum}")
            }
        };

        match generate_text_preview(path, lnum, self.preview_height) {
            Ok(TextPreview {
                start,
                end,
                total,
                highlight_lnum,
                lines,
            }) => {
                let maybe_code_context =
                    fetch_code_context(&lines, highlight_lnum, lnum, start, path).await;

                // 1 (header line) + 1 (1-based line number)
                let line_number_offset = 1 + 1 + if maybe_code_context.is_some() { 3 } else { 0 };

                let highlight_source = SyntaxHighlighter {
                    context: HighlightingContext {
                        lines: lines.clone(),
                        path: path.to_path_buf(),
                        line_number_offset,
                        max_line_width: self.max_line_width(),
                        range: start..end + 1,
                        maybe_code_context: maybe_code_context.clone(),
                    },
                    timeout: 200,
                }
                .highlight_with_timeout()
                .await;

                let context_lines = maybe_code_context
                    .map(|code_context| {
                        code_context.format_for_display(container_width, self.ctx.env.is_nvim)
                    })
                    .unwrap_or_default();

                let context_lines_is_empty = context_lines.is_empty();

                let highlight_lnum = highlight_lnum + context_lines.len();

                let header_line = truncated_preview_header();
                let lines = std::iter::once(header_line)
                    .chain(context_lines.into_iter())
                    .chain(self.truncate_preview_lines(lines.into_iter()))
                    .collect::<Vec<_>>();

                let scrollbar = if self.ctx.env.should_add_scrollbar(total) {
                    let start = if context_lines_is_empty {
                        start.saturating_sub(3)
                    } else {
                        start
                    };

                    compute_scrollbar_position(self.ctx, start, end, total)
                } else {
                    None
                };

                let mut preview = Preview {
                    lines,
                    highlight_line: Some(HighlightLine {
                        line_number: highlight_lnum + 1,
                        column_range,
                    }),
                    scrollbar,
                    ..Default::default()
                };

                preview.set_highlights(highlight_source, path);

                preview
            }
            Err(err) => {
                tracing::error!(?path, provider_id = %self.ctx.provider_id(), "Couldn't read first lines: {err:?}");
                let header_line = truncated_preview_header();
                let lines = vec![
                    header_line,
                    format!("Error while previewing {}: {err}", path.display()),
                ];
                Preview {
                    lines,
                    vim_syntax_info: VimSyntaxInfo::fname(fname),
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
                    let job_id = utils::compute_hash(&shell_cmd);

                    if job::reserve(job_id) {
                        let ctx = self.ctx.clone();

                        // TODO: Refresh with a timeout.
                        tokio::task::spawn_blocking(move || {
                            tracing::debug!(cwd = ?ctx.cwd, "Refreshing grep cache");
                            let new_digest = match crate::tools::rg::refresh_cache(&ctx.cwd) {
                                Ok(digest) => {
                                    tracing::debug!(total = digest.total, "Refreshed grep cache");
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
                            "Another grep job is running, skip freshing the cache"
                        );
                    }
                }
            }
        }
    }

    /// Truncates the lines that are awfully long as vim might have some performance issue with
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
        2 * self.ctx.env.display_line_width
    }
}

enum FileClass {
    NotRegularFile,
    Binary,
    Text(utils::io::FileSizeTier),
}

fn detect_file_class(path: &Path) -> std::io::Result<FileClass> {
    if !path.is_file() {
        return Ok(FileClass::NotRegularFile);
    }

    let mut file = std::fs::File::open(&path)?;
    let metadata = file.metadata()?;

    let mut buf = vec![0u8; 1024];
    let n = file.read(&mut buf)?;
    let content_type = content_inspector::inspect(&buf[..n]);

    if content_type.is_binary() {
        return Ok(FileClass::Binary);
    }

    let file_size_tier = utils::io::FileSizeTier::from_metadata(&metadata);
    Ok(FileClass::Text(file_size_tier))
}

async fn fetch_context_tag_with_timeout(path: &Path, lnum: usize) -> Option<BufferTag> {
    let (tag_sender, tag_receiver) = oneshot::channel();

    const TIMEOUT: Duration = Duration::from_millis(200);

    std::thread::spawn({
        let path = path.to_path_buf();
        move || {
            let result = current_context_tag(&path, lnum);
            let _ = tag_sender.send(result);
        }
    });

    match tokio::time::timeout(TIMEOUT, tag_receiver).await {
        Ok(res) => res.ok().flatten(),
        Err(_) => {
            tracing::debug!(timeout = ?TIMEOUT, ?path, lnum, "⏳ Timeout fetching context tag");
            None
        }
    }
}

/// This struct represents the context of current code block in the preview window.
///
/// It's likely that the displayed preview content is unable to convey the context due to the size
/// limit of the preview window, for instance, a function is too large to fit into the preview
/// window and the cursor is in the middle of this function, we're missing the context which function
/// the code block is in.
///
/// The idea here is similar to how we find the nearest symbol at cursor using ctags, finding the
/// line containing the context line and displaying it along with the normal preview content.
#[derive(Clone)]
struct CodeContext {
    /// Full context line.
    ///
    /// `async fn fetch_context_lines(`
    line: String,
}

impl CodeContext {
    const CONTEXT_LINE_NUMBER: usize = 2;
    // 1 context line + 2 border lines.
    const CONTEXT_LINES_LEN: usize = 3;

    /// Converts the context into series of formatted lines for display.
    ///
    /// ------------------
    /// line
    /// ------------------
    fn format_for_display(self, container_width: usize, is_nvim: bool) -> Vec<String> {
        // Vim has a different border width.
        let border_line = "─".repeat(if is_nvim {
            container_width
        } else {
            container_width - 2
        });

        let mut context_lines = Vec::with_capacity(Self::CONTEXT_LINES_LEN);

        context_lines.push(border_line.clone());

        // Truncate the right of pattern, 2 whitespaces + 💡
        let max_line_len = container_width - 4;
        let mut line = self.line;
        if line.len() > max_line_len {
            // Use the chars instead of indexing the str to avoid the char boundary error.
            line = line.chars().take(max_line_len - 4 - 2).collect::<String>();
            line.push_str("..");
        };
        line.push_str("  💡");
        context_lines.push(line);

        context_lines.push(border_line);

        context_lines
    }
}

async fn fetch_code_context(
    lines: &[String],
    highlight_lnum: usize,
    lnum: usize,
    start: usize,
    path: &Path,
) -> Option<CodeContext> {
    // Some checks against the latest preview line.
    let line = lines.get(highlight_lnum - 1)?;
    let ext = path.extension().and_then(|e| e.to_str())?;

    let skip_context_tag = {
        const BLACK_LIST: &[&str] = &["log", "txt", "lock", "toml", "yaml", "mod", "conf"];

        BLACK_LIST.contains(&ext) || code_tools::language::is_comment(line, ext)
    };

    if skip_context_tag {
        return None;
    };

    match fetch_context_tag_with_timeout(path, lnum).await {
        Some(tag) if tag.line_number < start => Some(CodeContext {
            line: tag.trimmed_pattern().to_string(),
        }),
        _ => {
            // No context lines if no tag found prior to the line number.
            None
        }
    }
}

fn compute_scrollbar_position(
    ctx: &Context,
    start: usize,
    end: usize,
    total: usize,
) -> Option<(usize, usize)> {
    let preview_winheight = ctx.env.display_winheight;

    let length = (((end - start) * preview_winheight) as f32 / total as f32) as usize;

    let top_position = (start * preview_winheight) as f32 / total as f32;

    if length == 0 {
        None
    } else {
        let mut length = preview_winheight.min(length);
        let top_position = if ctx.env.preview_border_enabled {
            length -= if length == preview_winheight { 1 } else { 0 };

            top_position.max(1.0) as usize
        } else {
            top_position as usize
        };

        Some((top_position, length))
    }
}

enum HighlightSource {
    Sublime(SublimeHighlightData),
    TreeSitter(TreeSitterHighlightData),
    None,
}

struct HighlightingContext {
    lines: Vec<String>,
    path: PathBuf,
    line_number_offset: usize,
    max_line_width: usize,
    range: Range<usize>,
    maybe_code_context: Option<CodeContext>,
}

struct SyntaxHighlighter {
    context: HighlightingContext,
    // Timeout in milliseconds.
    timeout: u64,
}

impl SyntaxHighlighter {
    // Fetch with highlights with a timeout.
    //
    // `compute_syntax_highlighting` might be slow for larger files (over 100k lines) as tree-sitter will
    // have to parse the whole file to obtain the highlight info. Therefore, we must run the actual
    // worker in a separated task to not make the async runtime blocked, otherwise we may run into
    // the issue of frozen UI.
    async fn highlight_with_timeout(self) -> HighlightSource {
        let (result_sender, result_receiver) = oneshot::channel();

        let Self { context, timeout } = self;

        let path = context.path.clone();

        std::thread::spawn({
            move || {
                let result = compute_syntax_highlighting(context);
                let _ = result_sender.send(result);
            }
        });

        match tokio::time::timeout(Duration::from_millis(timeout), result_receiver).await {
            Ok(res) => res.unwrap_or(HighlightSource::None),
            Err(_) => {
                tracing::debug!(?timeout, ?path, "⏳ Timeout fetching preview highlights");
                HighlightSource::None
            }
        }
    }
}

// TODO: this might be slow for larger files (over 100k lines) as tree-sitter will have to
// parse the whole file to obtain the highlight info. We may make the highlighting async.
fn compute_syntax_highlighting(context: HighlightingContext) -> HighlightSource {
    let HighlightingContext {
        lines,
        path,
        line_number_offset,
        max_line_width,
        range,
        maybe_code_context,
    } = context;

    match maple_config::config().provider.preview_highlight_engine {
        HighlightEngine::SublimeSyntax => {
            sublime_highlighting(&lines, &path, line_number_offset, max_line_width)
        }
        HighlightEngine::TreeSitter => {
            tree_sitter_highlighting(&path, range, max_line_width, maybe_code_context.as_ref())
        }
        HighlightEngine::Vim => HighlightSource::None,
    }
}

fn sublime_highlighting(
    lines: &[String],
    path: &Path,
    line_number_offset: usize,
    max_line_width: usize,
) -> HighlightSource {
    const THEME: &str = "Visual Studio Dark+";

    let theme = match &maple_config::config().provider.sublime_syntax_color_scheme {
        Some(theme) => {
            if sublime_theme_exists(theme) {
                theme.as_str()
            } else {
                tracing::warn!("preview color theme {theme} not found, fallback to {THEME}");
                THEME
            }
        }
        None => THEME,
    };

    path.extension()
        .and_then(|s| s.to_str())
        .and_then(sublime_syntax_by_extension)
        .map(|syntax| {
            //  Same reason as [`Self::truncate_preview_lines()`], if a line is too
            //  long and the query is short, the highlights can be enomerous and
            //  cause the Vim frozen due to the too many highlight works.
            let max_len = max_line_width;
            let lines = lines.iter().map(|s| {
                let len = s.len().min(max_len);
                &s[..len]
            });
            sublime_syntax_highlight(syntax, lines, line_number_offset, theme)
        })
        .map(HighlightSource::Sublime)
        .unwrap_or(HighlightSource::None)
}

fn tree_sitter_highlighting(
    path: &Path,
    visible_range: Range<usize>,
    max_line_width: usize,
    code_context: Option<&CodeContext>,
) -> HighlightSource {
    const FILE_SIZE_CHECKER: SizeChecker = SizeChecker::new(1024 * 1024);

    if FILE_SIZE_CHECKER.is_too_large(path).unwrap_or(true) {
        return HighlightSource::None;
    }

    tree_sitter::Language::try_from_path(path)
        .and_then(|language| {
            let Ok(source_code) = std::fs::read(path) else {
                return None;
            };

            // TODO: Cache the highlights per one provider session or even globally?
            // 1. Check the last modified time.
            // 2. If unchanged, try retrieving from the cache.
            // 3. Otherwise parse it.
            let Ok(raw_highlights) = language.highlight(&source_code) else {
                return None;
            };

            let line_start = visible_range.start;

            let ts_highlights = convert_raw_ts_highlights_to_vim_highlights(
                &raw_highlights,
                language,
                visible_range.into(),
            );

            let mut maybe_context_line_highlight = None;

            let context_lines_offset = if let Some(code_context) = code_context {
                if let Ok(highlight_items) = language.highlight_line(code_context.line.as_bytes()) {
                    let line_highlights = highlight_items
                        .into_iter()
                        .filter_map(|i| {
                            let start = i.start.column;
                            let length = i.end.column - i.start.column;
                            // Ignore the invisible highlights.
                            if start + length > max_line_width {
                                None
                            } else {
                                let group = language.highlight_group(i.highlight);
                                Some((start, length, group.to_string()))
                            }
                        })
                        .collect::<Vec<_>>();

                    maybe_context_line_highlight
                        .replace((CodeContext::CONTEXT_LINE_NUMBER, line_highlights));
                }

                CodeContext::CONTEXT_LINES_LEN
            } else {
                0
            };

            let tree_sitter_highlight_data = ts_highlights
                .into_iter()
                .map(|(line_number, line_highlights)| {
                    let line_number_in_preview_win =
                        line_number - line_start + 1 + context_lines_offset;

                    // Workaround the lifetime issue, nice to remove this allocation
                    // `group.to_string()` as it's essentially `&'static str`.
                    let line_highlights = line_highlights
                        .into_iter()
                        .filter_map(|(start, length, group)| {
                            // Ignore the invisible highlights.
                            if start + length > max_line_width {
                                None
                            } else {
                                Some((start, length, group.to_string()))
                            }
                        })
                        .collect();

                    (line_number_in_preview_win, line_highlights)
                })
                .chain(maybe_context_line_highlight)
                .collect::<Vec<_>>();

            Some(tree_sitter_highlight_data)
        })
        .map(HighlightSource::TreeSitter)
        .unwrap_or(HighlightSource::None)
}
