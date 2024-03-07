mod hooks;
mod impls;

use self::hooks::{initialize_provider, CachedPreviewImpl, Preview, PreviewTarget};
use crate::searcher::file::BlinesItem;
use crate::searcher::SearchContext;
use crate::stdio_server::input::{
    InputRecorder, InternalProviderEvent, KeyEvent, KeyEventType, ProviderEvent,
};
use crate::stdio_server::vim::{Vim, VimError, VimResult};
use filter::Query;
use icon::{Icon, IconKind};
use matcher::{Bonus, MatchScope, Matcher, MatcherBuilder};
use once_cell::sync::OnceCell;
use parking_lot::RwLock;
use paths::AbsPathBuf;
use printer::Printer;
use rpc::Params;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::fmt::Debug;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc::UnboundedSender;
use types::{ClapItem, MatchedItem};

pub use self::impls::filer::read_dir_entries;
pub use self::impls::{create_provider, lsp};

#[derive(Debug, thiserror::Error)]
pub enum ProviderError {
    #[error("can not find the item at line_number {line_number}")]
    PreviewItemNotFound { line_number: usize },
    #[error("can not scroll the preview as preview target does not exist")]
    PreviewTargetNotFound,
    #[error("preview scroll is only available for the file preview target")]
    OnlyFilePreviewScrollSupported,
    #[error("line number is larger than total lines")]
    ExceedingMaxLines(usize, usize),
    #[error("failed to convert {0} to absolute path")]
    ConvertToAbsolutePath(String),
    #[error("{0}")]
    Other(String),
    #[error(transparent)]
    Vim(#[from] VimError),
    #[error(transparent)]
    Join(#[from] tokio::task::JoinError),
    #[error(transparent)]
    SendProviderEvent(#[from] tokio::sync::mpsc::error::SendError<ProviderEvent>),
    #[error(transparent)]
    IO(#[from] std::io::Error),
    #[error(transparent)]
    ParseInt(#[from] std::num::ParseIntError),
}

pub type ProviderResult<T> = std::result::Result<T, ProviderError>;

/// [`BaseArgs`] represents the arguments common to all the providers.
#[derive(Debug, Clone, clap::Parser, PartialEq, Eq, Default)]
pub struct BaseArgs {
    /// Specify the initial query.
    #[clap(long)]
    query: Option<String>,

    /// Specify the working directory for this provider and all subsequent providers.
    #[clap(long)]
    cwd: Option<PathBuf>,

    /// Skip the default working directory in searching.
    #[clap(long)]
    no_cwd: bool,
}

#[derive(Debug)]
struct SearcherControl {
    stop_signal: Arc<AtomicBool>,
    join_handle: tokio::task::JoinHandle<()>,
}

impl SearcherControl {
    fn kill(self) {
        self.stop_signal.store(true, Ordering::SeqCst);
        self.join_handle.abort();
    }
}

/// bufnr and winid.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BufnrWinid {
    pub bufnr: usize,
    pub winid: usize,
}

#[derive(Debug, Clone)]
pub enum PreviewDirection {
    /// LR
    LeftRight,
    /// UD
    UpDown,
    /// Auto
    Auto,
}

impl PreviewDirection {
    pub fn is_left_right(&self) -> bool {
        matches!(self, Self::LeftRight)
    }
}

/// Provider environment initialized at invoking the provider.
///
/// Immutable once initialized.
#[derive(Debug, Clone)]
pub struct ProviderEnvironment {
    pub is_nvim: bool,
    pub has_nvim_09: bool,
    pub provider_id: ProviderId,
    pub start: BufnrWinid,
    pub input: BufnrWinid,
    pub display: BufnrWinid,
    pub icon: Icon,
    pub matcher_builder: MatcherBuilder,
    pub no_cache: bool,
    pub source_is_list: bool,
    pub preview_enabled: bool,
    pub preview_border_enabled: bool,
    pub preview_direction: PreviewDirection,
    pub display_winwidth: usize,
    pub display_winheight: usize,
    /// Actual width for displaying the line content due to the sign column is included in
    /// winwidth.
    pub display_line_width: usize,
    pub start_buffer_path: PathBuf,
}

impl ProviderEnvironment {
    /// Returns `true` if the scrollbar should be added to the preview window.
    pub fn should_add_scrollbar(&self, total: usize) -> bool {
        self.is_nvim && self.preview_direction.is_left_right() && total > 0
    }
}

#[derive(Debug, Clone)]
pub enum Direction {
    Down,
    Up,
}

#[derive(Debug, Clone, Copy)]
struct ScrollFile {
    line_start: usize,
    total_lines: usize,
}

impl ScrollFile {
    fn new(line_start: usize, path: &Path) -> std::io::Result<Self> {
        Ok(Self {
            line_start,
            total_lines: utils::line_count(path)?,
        })
    }
}

#[derive(Debug, Clone)]
pub struct PreviewManager {
    scroll_file: Option<ScrollFile>,
    scroll_offset: i32,
    current_preview_target: Option<PreviewTarget>,
    preview_cache: Arc<RwLock<HashMap<PreviewTarget, Preview>>>,
}

impl PreviewManager {
    const SCROLL_SIZE: i32 = 10;

    pub fn new() -> Self {
        Self {
            scroll_file: None,
            scroll_offset: 0,
            current_preview_target: None,
            preview_cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn cached_preview(&self, preview_target: &PreviewTarget) -> Option<Preview> {
        let preview_cache = self.preview_cache.read();
        // TODO: not clone?
        preview_cache.get(preview_target).cloned()
    }

    pub fn insert_preview(&self, preview_target: PreviewTarget, preview: Preview) {
        let mut preview_cache = self.preview_cache.write();
        preview_cache.insert(preview_target, preview);
    }

    fn reset_scroll(&mut self) {
        self.scroll_file.take();
        self.scroll_offset = 0;
        self.current_preview_target.take();
    }

    fn prepare_scroll_file_info(
        &mut self,
        line_start: usize,
        path: PathBuf,
    ) -> std::io::Result<(ScrollFile, PathBuf)> {
        let scroll_file = match self.scroll_file {
            Some(scroll_file) => scroll_file,
            None => {
                let scroll_file = ScrollFile::new(line_start, &path)?;
                self.scroll_file.replace(scroll_file);
                scroll_file
            }
        };
        Ok((scroll_file, path))
    }

    fn set_preview_target(&mut self, preview_target: PreviewTarget) {
        self.current_preview_target.replace(preview_target);
    }

    fn scroll_preview(&mut self, direction: Direction) -> ProviderResult<PreviewTarget> {
        let new_scroll_offset = match direction {
            Direction::Up => self.scroll_offset - 1,
            Direction::Down => self.scroll_offset + 1,
        };

        let (scroll_file, path) = match self
            .current_preview_target
            .as_ref()
            .ok_or(ProviderError::PreviewTargetNotFound)?
        {
            PreviewTarget::LineInFile { path, line_number } => {
                self.prepare_scroll_file_info(*line_number, path.clone())?
            }
            PreviewTarget::File(path) => self.prepare_scroll_file_info(0, path.clone())?,
            _ => return Err(ProviderError::OnlyFilePreviewScrollSupported),
        };

        let ScrollFile {
            line_start,
            total_lines,
        } = scroll_file;

        let new_line_number = line_start as i32 + new_scroll_offset * Self::SCROLL_SIZE;

        let new_line_number = if new_line_number < 0 {
            // Reaching the start of file.
            0
        } else if new_line_number as usize > total_lines {
            return Err(ProviderError::ExceedingMaxLines(
                new_line_number as usize,
                total_lines,
            ));
        } else {
            self.scroll_offset = new_scroll_offset;
            new_line_number
        };

        let new_target = PreviewTarget::LineInFile {
            path,
            line_number: new_line_number as usize,
        };

        Ok(new_target)
    }
}

#[derive(Debug, Clone)]
pub struct Context {
    pub cwd: AbsPathBuf,
    pub vim: Vim,
    pub env: Arc<ProviderEnvironment>,
    pub maybe_preview_size: Option<usize>,
    pub initializing_prompt_echoed: Arc<AtomicBool>,
    pub terminated: Arc<AtomicBool>,
    pub input_recorder: InputRecorder,
    pub preview_manager: PreviewManager,
    pub provider_source: Arc<RwLock<ProviderSource>>,
    provider_event_sender: OnceCell<UnboundedSender<ProviderEvent>>,
}

impl Context {
    pub async fn new(params: Params, vim: Vim) -> VimResult<Self> {
        #[derive(Deserialize)]
        struct InitializeParams {
            provider_id: ProviderId,
            start: BufnrWinid,
            input: BufnrWinid,
            display: BufnrWinid,
            cwd: AbsPathBuf,
            icon: String,
            no_cache: bool,
            start_buffer_path: PathBuf,
            source_is_list: bool,
        }

        let InitializeParams {
            provider_id,
            start,
            input,
            display,
            cwd,
            no_cache,
            start_buffer_path,
            icon,
            source_is_list,
        } = params.parse()?;

        let icon = match icon.to_lowercase().as_str() {
            "file" => Icon::Enabled(IconKind::File),
            "grep" => Icon::Enabled(IconKind::Grep),
            "projtags" => Icon::Enabled(IconKind::ProjTags),
            "buffertags" => Icon::Enabled(IconKind::BufferTags),
            "lsp" => Icon::ClapItem,
            _ => Icon::Null,
        };

        let rank_criteria = maple_config::config().matcher.rank_criteria();
        let matcher_builder = provider_id.matcher_builder().rank_criteria(rank_criteria);

        let display_winwidth = vim.winwidth(display.winid).await?;
        let display_winheight = vim.winheight(display.winid).await?;
        let is_nvim = vim.call::<usize>("has", ["nvim"]).await? == 1;
        let has_nvim_09 = vim.call::<usize>("has", ["nvim-0.9"]).await? == 1;
        let preview_enabled = vim.bare_call::<usize>("clap#preview#is_enabled").await? == 1;
        let preview_direction = {
            let preview_direction: String = vim.bare_call("clap#preview#direction").await?;
            match preview_direction.to_uppercase().as_str() {
                "LR" => PreviewDirection::LeftRight,
                "UD" => PreviewDirection::UpDown,
                _ => PreviewDirection::Auto,
            }
        };
        let popup_border: String = vim.eval("g:clap_popup_border").await?;

        // Sign column occupies 2 spaces.
        let mut display_line_width = display_winwidth - 2;
        if provider_id.as_str() == "grep" {
            display_line_width -= 2;
        }

        let env = ProviderEnvironment {
            is_nvim,
            has_nvim_09,
            provider_id,
            start,
            input,
            display,
            no_cache,
            source_is_list,
            preview_enabled,
            preview_border_enabled: popup_border != "nil",
            preview_direction,
            start_buffer_path,
            display_winwidth,
            display_winheight,
            display_line_width,
            matcher_builder,
            icon,
        };

        let input_history = crate::datastore::INPUT_HISTORY_IN_MEMORY.lock();
        let inputs = if maple_config::config().provider.share_input_history {
            input_history.all_inputs()
        } else {
            input_history.inputs(&env.provider_id)
        };
        let input_recorder = InputRecorder::new(inputs);

        Ok(Self {
            cwd,
            vim,
            env: Arc::new(env),
            maybe_preview_size: None,
            initializing_prompt_echoed: Arc::new(AtomicBool::new(false)),
            terminated: Arc::new(AtomicBool::new(false)),
            input_recorder,
            preview_manager: PreviewManager::new(),
            provider_source: Arc::new(RwLock::new(ProviderSource::Uninitialized)),
            provider_event_sender: OnceCell::new(),
        })
    }

    pub fn provider_id(&self) -> &str {
        self.env.provider_id.as_str()
    }

    pub fn provider_debounce(&self) -> u64 {
        maple_config::config().provider_debounce(self.env.provider_id.as_str())
    }

    pub fn matcher_builder(&self) -> MatcherBuilder {
        self.env.matcher_builder.clone()
    }

    pub fn matcher(&self, query: impl Into<Query>) -> Matcher {
        self.env.matcher_builder.clone().build(query.into())
    }

    /// Constructs a [`SearchContext`] for the searching worker.
    pub fn search_context(&self, stop_signal: Arc<AtomicBool>) -> SearchContext {
        SearchContext {
            icon: self.env.icon,
            line_width: self.env.display_line_width,
            paths: vec![self.cwd.to_path_buf()],
            vim: self.vim.clone(),
            stop_signal,
            item_pool_size: self.env.display_winheight,
        }
    }

    pub fn set_provider_event_sender(&self, provider_event_sender: UnboundedSender<ProviderEvent>) {
        self.provider_event_sender
            .set(provider_event_sender)
            .expect("Failed to initialize provider_event_sender in Context")
    }

    pub fn send_provider_event(&self, event: ProviderEvent) -> ProviderResult<()> {
        self.provider_event_sender
            .get()
            .expect("Forget to initialize provider_event_sender!")
            .send(event)
            .map_err(Into::into)
    }

    /// Executes the command `cmd` and returns the raw bytes of stdout.
    pub fn exec_cmd(&self, cmd: &str) -> std::io::Result<Vec<u8>> {
        let out = utils::execute_at(cmd, Some(&self.cwd))?;
        Ok(out.stdout)
    }

    pub fn start_buffer_extension(&self) -> std::io::Result<&str> {
        self.env
            .start_buffer_path
            .extension()
            .and_then(|s| s.to_str())
            .ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!(
                        "Extension not found for start_buffer_path: {}",
                        self.env.start_buffer_path.display()
                    ),
                )
            })
    }

    pub async fn parse_provider_args<T: clap::Parser + Default + Debug>(&self) -> VimResult<T> {
        let args = self.vim.provider_args().await?;

        let provider_args = if args.is_empty() {
            T::default()
        } else {
            T::try_parse_from(std::iter::once(String::from("")).chain(args.into_iter()))
                .map_err(|err| {
                    match err.kind() {
                        clap::error::ErrorKind::DisplayHelp => {
                            // Show help in the display window.
                            let err_msg = err.to_string();
                            let lines = err_msg.split('\n').collect::<Vec<_>>();
                            let _ = self.vim.exec("display_set_lines", [lines]);
                        }
                        _ => {
                            let _ = self.vim.echo_warn(format!(
                                "using default {:?} due to {err}",
                                T::default()
                            ));
                        }
                    }
                })
                .unwrap_or_default()
        };
        Ok(provider_args)
    }

    pub async fn handle_base_args(&self, base: &BaseArgs) -> ProviderResult<()> {
        let BaseArgs { query, .. } = base;

        if let Some(query) = query {
            self.send_provider_event(ProviderEvent::Internal(
                InternalProviderEvent::InitialQuery(query.clone()),
            ))?;
        };

        Ok(())
    }

    pub async fn expanded_paths(&self, paths: &[PathBuf]) -> VimResult<Vec<PathBuf>> {
        let mut expanded_paths = Vec::with_capacity(paths.len());
        for p in paths {
            if let Ok(path) = self.vim.expand(p.to_string_lossy()).await {
                expanded_paths.push(path.into());
            }
        }
        Ok(expanded_paths)
    }

    pub fn set_provider_source(&self, new: ProviderSource) {
        let mut provider_source = self.provider_source.write();
        *provider_source = new;
    }

    /// Returns a smaller delay for the input debounce if the source is not large.
    pub fn adaptive_debounce_delay(&self) -> Option<Duration> {
        if let ProviderSource::Small { total, .. } = *self.provider_source.read() {
            if total < 10_000 {
                return Some(Duration::from_millis(10));
            } else if total < 100_000 {
                return Some(Duration::from_millis(50));
            } else if total < 200_000 {
                return Some(Duration::from_millis(100));
            }
        }
        None
    }

    pub fn signify_terminated(&self, session_id: u64) {
        self.terminated.store(true, Ordering::SeqCst);
        let provider_id = self.env.provider_id.clone();
        tracing::debug!("ProviderSession {session_id:?}-{provider_id} terminated");
        let mut input_history = crate::datastore::INPUT_HISTORY_IN_MEMORY.lock();
        input_history.update_inputs(provider_id, self.input_recorder.clone().into_inputs());
        if let Err(err) = crate::datastore::store_input_history(&input_history) {
            tracing::error!(?err, "Failed to sync the latest input history to the disk.");
        }
    }

    pub async fn record_input(&mut self) -> VimResult<()> {
        let input = self.vim.input_get().await?;
        self.input_recorder.try_record(input);
        Ok(())
    }

    /// Sets input to the next query.
    pub async fn next_input(&mut self) -> ProviderResult<()> {
        if let Some(next) = self.input_recorder.move_to_next() {
            if self.env.is_nvim {
                self.vim.exec("clap#picker#set_input", [next])?;
            } else {
                self.vim
                    .exec("clap#popup#move_manager#set_input_and_react", [next])?;
            }
        }
        Ok(())
    }

    /// Sets input to the previous query.
    pub async fn prev_input(&mut self) -> ProviderResult<()> {
        if let Some(previous) = self.input_recorder.move_to_prev() {
            if self.env.is_nvim {
                self.vim.exec("clap#picker#set_input", [previous])?;
            } else {
                self.vim
                    .exec("clap#popup#move_manager#set_input_and_react", [previous])?;
            }
        }
        Ok(())
    }

    pub async fn preview_size(&mut self) -> VimResult<usize> {
        match self.maybe_preview_size {
            Some(size) => Ok(size),
            None => {
                let preview_winid = self.vim.eval("g:clap.preview.winid").await?;
                let size = self
                    .vim
                    .preview_size(&self.env.provider_id, preview_winid)
                    .await?;
                self.maybe_preview_size.replace(size);
                Ok(size)
            }
        }
    }

    pub async fn preview_winwidth(&self) -> VimResult<usize> {
        let preview_winid = self.vim.eval("g:clap.preview.winid").await?;
        let winwidth = self.vim.winwidth(preview_winid).await?;
        Ok(winwidth)
    }

    pub async fn preview_height(&mut self) -> VimResult<usize> {
        self.preview_size().await.map(|x| 2 * x)
    }

    pub fn update_picker_preview(&self, preview: Preview) -> VimResult<()> {
        self.vim.exec("clap#picker#update_preview", preview)
    }

    async fn update_preview(
        &mut self,
        maybe_preview_target: Option<PreviewTarget>,
    ) -> ProviderResult<()> {
        let lnum = self.vim.display_getcurlnum().await?;

        let curline = self.vim.display_getcurline().await?;

        if curline.is_empty() {
            tracing::debug!("Skipping preview as curline is empty");
            self.vim.bare_exec("clap#picker#clear_preview")?;
            return Ok(());
        }

        let preview_height = self.preview_height().await?;

        let cached_preview_impl = if let Some(preview_target) = maybe_preview_target {
            CachedPreviewImpl::with_preview_target(preview_target, preview_height, self)
        } else {
            CachedPreviewImpl::new(curline, preview_height, self)?
        };

        let (preview_target, preview) = cached_preview_impl.get_preview().await?;

        // Ensure the preview result is not out-dated.
        let cur_lnum = self.vim.display_getcurlnum().await?;
        if cur_lnum == lnum {
            self.update_picker_preview(preview)?;
        }

        self.preview_manager
            .current_preview_target
            .replace(preview_target);

        Ok(())
    }

    async fn scroll_preview(&mut self, direction: Direction) -> ProviderResult<()> {
        if let Ok(new_preview_target) = self.preview_manager.scroll_preview(direction) {
            self.update_preview(Some(new_preview_target)).await?;
        }
        Ok(())
    }

    pub async fn update_on_empty_query(&self) -> VimResult<()> {
        if let Some(items) = self
            .provider_source
            .read()
            .try_skim(self.provider_id(), 100)
        {
            let printer = Printer::new(self.env.display_winwidth, self.env.icon);
            let printer::DisplayLines {
                lines,
                icon_added,
                truncated_map,
                ..
            } = printer.to_display_lines(items);

            self.vim.exec(
                "clap#picker#update_on_empty_query",
                json!([lines, truncated_map, icon_added]),
            )
        } else {
            self.vim.bare_exec("clap#picker#clear_all")
        }
    }
}

#[derive(Debug, Clone, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub struct ProviderId(String);

impl ProviderId {
    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn matcher_builder(&self) -> MatcherBuilder {
        let match_scope = match self.0.as_str() {
            "grep" | "live_grep" => MatchScope::GrepLine,
            "tags" | "proj_tags" => MatchScope::TagName,
            _ => MatchScope::Full,
        };

        let match_bonuses = match self.0.as_str() {
            "files" | "git_files" | "filer" => vec![Bonus::FileName],
            _ => vec![],
        };

        MatcherBuilder::new()
            .bonuses(match_bonuses)
            .match_scope(match_scope)
    }
}

impl<T: AsRef<str>> From<T> for ProviderId {
    fn from(s: T) -> Self {
        Self(s.as_ref().to_owned())
    }
}

impl std::fmt::Display for ProviderId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// This type represents the way to get the source items of a provider.
#[derive(Debug, Clone, Default)]
pub enum ProviderSource {
    /// The provider source is not actionable on the Rust backend.
    #[default]
    Uninitialized,

    /// The initialization is in progress,
    Initializing,

    /// Failed to initialize the source.
    InitializationFailed(String),

    /// Small scale, in which case we do not have to use the dynamic filtering.
    ///
    /// The scale can be small for some known swift providers or when a provider's source
    /// is a List or a function returning a List.
    Small {
        total: usize,
        items: Vec<Arc<dyn ClapItem>>,
    },

    /// The items originate from a normal file.
    File { total: usize, path: PathBuf },

    /// Read the items from a cache file created by vim-clap.
    ///
    /// The items from this file might be out-dated if not refreshed.
    CachedFile {
        total: usize,
        path: PathBuf,
        refreshed: bool,
    },

    /// Shell command to generate the source items.
    ///
    /// Execute the shell command to generate the source on each OnTyped event, the last run needs to
    /// be killed for sure before starting a new run.
    Command(String),
}

impl ProviderSource {
    pub fn total(&self) -> Option<usize> {
        match self {
            Self::Small { total, .. }
            | Self::File { total, .. }
            | Self::CachedFile { total, .. } => Some(*total),
            _ => None,
        }
    }

    pub fn using_cache(&self) -> bool {
        matches!(self, Self::CachedFile { refreshed, .. } if !refreshed)
    }

    pub fn try_skim(&self, provider_id: &str, n: usize) -> Option<Vec<MatchedItem>> {
        match self {
            Self::Small { ref items, .. } => Some(
                items
                    .iter()
                    .take(n)
                    .map(|item| MatchedItem::from(item.clone()))
                    .collect(),
            ),
            Self::File { ref path, .. } | Self::CachedFile { ref path, .. } => {
                let lines_iter = utils::read_first_lines(path, n).ok()?;
                if provider_id == "blines" {
                    let items = lines_iter
                        .enumerate()
                        .map(|(index, line)| {
                            let item: Arc<dyn ClapItem> = Arc::new(BlinesItem {
                                raw: line,
                                line_number: index + 1,
                            });
                            MatchedItem::from(item)
                        })
                        .collect();
                    Some(items)
                } else {
                    let items = lines_iter
                        .map(|line| MatchedItem::from(Arc::new(line) as Arc<dyn ClapItem>))
                        .collect();
                    Some(items)
                }
            }
            _ => None,
        }
    }
}

/// A trait each Clap provider must implement.
#[async_trait::async_trait]
pub trait ClapProvider: Debug + Send + Sync + 'static {
    async fn on_initialize(&mut self, ctx: &mut Context) -> ProviderResult<()> {
        initialize_provider(ctx, true).await
    }

    async fn on_initial_query(
        &mut self,
        ctx: &mut Context,
        initial_query: String,
    ) -> ProviderResult<()> {
        // Mimic the user behavior by setting the user input and sending the signal
        let _ = ctx
            .vim
            .call::<String>("set_initial_query", json!([initial_query]))
            .await;
        ctx.send_provider_event(ProviderEvent::OnTyped(Params::None))
    }

    async fn on_move(&mut self, ctx: &mut Context) -> ProviderResult<()> {
        if !ctx.env.preview_enabled {
            return Ok(());
        }
        ctx.preview_manager.reset_scroll();
        ctx.update_preview(None).await
    }

    async fn on_typed(&mut self, ctx: &mut Context) -> ProviderResult<()>;

    /// Handle the sink on the Rust side instead of the Vim side.
    async fn remote_sink(
        &mut self,
        _ctx: &mut Context,
        _line_numbers: Vec<usize>,
    ) -> ProviderResult<()> {
        Ok(())
    }

    /// On receiving the Terminate event.
    ///
    /// Sets the running signal to false, in case of the forerunner thread is still working.
    fn on_terminate(&mut self, ctx: &mut Context, session_id: u64) {
        ctx.signify_terminated(session_id);
    }

    async fn on_key_event(&mut self, ctx: &mut Context, key_event: KeyEvent) -> ProviderResult<()> {
        let (key_event_type, _params) = key_event;
        match key_event_type {
            KeyEventType::ShiftUp => ctx.scroll_preview(Direction::Up).await?,
            KeyEventType::ShiftDown => ctx.scroll_preview(Direction::Down).await?,
            KeyEventType::CtrlN => ctx.next_input().await?,
            KeyEventType::CtrlP => ctx.prev_input().await?,
            _ => {}
        }
        Ok(())
    }
}
