mod blines;
mod dumb_jump;
mod filer;
mod files;
mod generic_provider;
mod grep;
// mod interactive_grep;
mod recent_files;

pub use self::filer::read_dir_entries;
use crate::paths::AbsPathBuf;
use crate::searcher::blines::BlinesItem;
use crate::searcher::SearchContext;
use crate::stdio_server::handler::{
    initialize_provider, CachedPreviewImpl, Preview, PreviewTarget,
};
use crate::stdio_server::input::{InputRecorder, KeyEvent};
use crate::stdio_server::rpc::Params;
use crate::stdio_server::vim::Vim;
use anyhow::Result;
use filter::Query;
use icon::{Icon, IconKind};
use matcher::{Bonus, MatchScope, Matcher, MatcherBuilder};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::fmt::Debug;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use types::{ClapItem, MatchedItem};

pub async fn create_provider(provider_id: &str, ctx: &Context) -> Result<Box<dyn ClapProvider>> {
    let provider: Box<dyn ClapProvider> = match provider_id {
        "blines" => Box::new(blines::BlinesProvider::new()),
        "dumb_jump" => Box::new(dumb_jump::DumbJumpProvider::new()),
        "filer" => Box::new(filer::FilerProvider::new(ctx.cwd.to_path_buf())),
        "files" => Box::new(files::FilesProvider::new(ctx).await?),
        "grep" => Box::new(grep::GrepProvider::new()),
        // "interactive_grep" => Box::new(interactive_grep::InteractiveGrepProvider::new(
        // ctx.cwd.to_path_buf(),
        // )),
        "recent_files" => Box::new(recent_files::RecentFilesProvider::new()),
        _ => Box::new(generic_provider::GenericProvider::new()),
    };
    Ok(provider)
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

/// Provider environment initialized at invoking the provider.
///
/// Immutable once initialized.
#[derive(Debug, Clone)]
pub struct ProviderEnvironment {
    pub is_nvim: bool,
    pub provider_id: ProviderId,
    pub start: BufnrWinid,
    pub input: BufnrWinid,
    pub display: BufnrWinid,
    pub icon: Icon,
    pub matcher_builder: MatcherBuilder,
    pub debounce: bool,
    pub no_cache: bool,
    pub preview_enabled: bool,
    pub display_winwidth: usize,
    pub display_winheight: usize,
    pub start_buffer_path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct Context {
    pub cwd: AbsPathBuf,
    pub vim: Vim,
    pub env: Arc<ProviderEnvironment>,
    pub maybe_preview_size: Option<usize>,
    pub terminated: Arc<AtomicBool>,
    pub preview_cache: Arc<RwLock<HashMap<PreviewTarget, Preview>>>,
    pub input_recorder: InputRecorder,
    pub provider_source: Arc<RwLock<ProviderSource>>,
}

impl Context {
    pub async fn new(params: Params, vim: Vim) -> Result<Self> {
        #[derive(Deserialize)]
        struct InnerParams {
            provider_id: ProviderId,
            start: BufnrWinid,
            input: BufnrWinid,
            display: BufnrWinid,
            cwd: AbsPathBuf,
            icon: String,
            debounce: bool,
            no_cache: bool,
            start_buffer_path: PathBuf,
        }

        let InnerParams {
            provider_id,
            start,
            input,
            display,
            cwd,
            debounce,
            no_cache,
            start_buffer_path,
            icon,
        } = params.parse()?;

        let icon = match icon.to_lowercase().as_str() {
            "file" => Icon::Enabled(IconKind::File),
            "grep" => Icon::Enabled(IconKind::Grep),
            "projtags" => Icon::Enabled(IconKind::ProjTags),
            "buffertags" => Icon::Enabled(IconKind::BufferTags),
            _ => Icon::Null,
        };

        let rank_criteria = crate::config::config()
            .matcher
            .tiebreak
            .split(',')
            .filter_map(|s| types::parse_criteria(s.trim()))
            .collect();
        let matcher_builder = provider_id.matcher_builder().rank_criteria(rank_criteria);

        let display_winwidth = vim.winwidth(display.winid).await?;
        let display_winheight = vim.winheight(display.winid).await?;
        let is_nvim: usize = vim.call("has", ["nvim"]).await?;
        let preview_enabled: usize = vim.bare_call("clap#preview#is_enabled").await?;

        let input_history = crate::datastore::INPUT_HISTORY_IN_MEMORY.lock();
        let input_recorder = InputRecorder::new(input_history.inputs(&provider_id));

        let env = ProviderEnvironment {
            is_nvim: is_nvim == 1,
            provider_id,
            start,
            input,
            display,
            no_cache,
            debounce,
            preview_enabled: preview_enabled == 1,
            start_buffer_path,
            display_winwidth,
            display_winheight,
            matcher_builder,
            icon,
        };

        Ok(Self {
            cwd,
            vim,
            env: Arc::new(env),
            maybe_preview_size: None,
            terminated: Arc::new(AtomicBool::new(false)),
            preview_cache: Arc::new(RwLock::new(HashMap::new())),
            input_recorder,
            provider_source: Arc::new(RwLock::new(ProviderSource::Unactionable)),
        })
    }

    pub fn provider_id(&self) -> &str {
        self.env.provider_id.as_str()
    }

    pub fn matcher_builder(&self) -> MatcherBuilder {
        self.env.matcher_builder.clone()
    }

    pub fn matcher(&self, query: impl Into<Query>) -> Matcher {
        self.env.matcher_builder.clone().build(query.into())
    }

    pub fn search_context(&self, stop_signal: Arc<AtomicBool>) -> SearchContext {
        SearchContext {
            icon: self.env.icon,
            winwidth: self.env.display_winwidth,
            paths: vec![self.cwd.to_path_buf()],
            vim: self.vim.clone(),
            stop_signal,
            item_pool_size: self.env.display_winheight,
        }
    }

    /// Executes the command `cmd` and returns the raw bytes of stdout.
    pub fn exec_cmd(&self, cmd: &str) -> std::io::Result<Vec<u8>> {
        let out = utils::execute_at(cmd, Some(&self.cwd))?;
        Ok(out.stdout)
    }

    pub fn start_buffer_extension(&self) -> std::io::Result<String> {
        self.env
            .start_buffer_path
            .extension()
            .and_then(|s| s.to_str())
            .map(|s| s.to_string())
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

    pub fn set_provider_source(&self, new: ProviderSource) {
        let mut provider_source = self.provider_source.write();
        *provider_source = new;
    }

    pub fn signify_terminated(&self, session_id: u64) {
        self.terminated.store(true, Ordering::SeqCst);
        let mut input_history = crate::datastore::INPUT_HISTORY_IN_MEMORY.lock();
        input_history.insert(
            self.env.provider_id.clone(),
            self.input_recorder.clone().into_inputs(),
        );
        tracing::debug!("Session {session_id:?}-{} terminated", self.provider_id());
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

    pub async fn record_input(&mut self) -> Result<()> {
        let input = self.vim.input_get().await?;
        self.input_recorder.try_record(input);
        Ok(())
    }

    pub async fn next_input(&mut self) -> Result<()> {
        if let Some(next) = self.input_recorder.move_to_next() {
            if self.env.is_nvim {
                self.vim.exec("clap#state#set_input", json!([next]))?;
            } else {
                self.vim
                    .exec("clap#popup#move_manager#set_input_and_react", json!([next]))?;
            }
        }
        Ok(())
    }

    pub async fn previous_input(&mut self) -> Result<()> {
        if let Some(previous) = self.input_recorder.move_to_previous() {
            if self.env.is_nvim {
                self.vim.exec("clap#state#set_input", json!([previous]))?;
            } else {
                self.vim.exec(
                    "clap#popup#move_manager#set_input_and_react",
                    json!([previous]),
                )?;
            }
        }
        Ok(())
    }

    pub async fn preview_size(&mut self) -> Result<usize> {
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

    pub async fn preview_height(&mut self) -> Result<usize> {
        self.preview_size().await.map(|x| 2 * x)
    }

    pub fn render_preview(&self, preview: Preview) -> Result<()> {
        self.vim.exec("clap#state#render_preview", preview)
    }

    pub async fn update_preview(&mut self) -> Result<()> {
        let lnum = self.vim.display_getcurlnum().await?;

        let curline = self.vim.display_getcurline().await?;

        if curline.is_empty() {
            tracing::debug!("Skipping preview as curline is empty");
            self.vim.bare_exec("clap#state#clear_preview")?;
            return Ok(());
        }

        let preview_height = self.preview_height().await?;

        let preview = CachedPreviewImpl::new(curline, preview_height, self)?
            .get_preview()
            .await?;

        // Ensure the preview result is not out-dated.
        let cur_lnum = self.vim.display_getcurlnum().await?;
        if cur_lnum == lnum {
            self.render_preview(preview)?;
        }

        Ok(())
    }

    pub async fn update_on_empty_query(&self) -> Result<()> {
        if let Some(items) = self
            .provider_source
            .read()
            .try_skim(self.provider_id(), 100)
        {
            let printer::DisplayLines {
                lines,
                icon_added,
                truncated_map,
                ..
            } = printer::to_display_lines(items, self.env.display_winwidth, self.env.icon);

            self.vim.exec(
                "clap#state#update_on_empty_query",
                json!([lines, truncated_map, icon_added]),
            )
        } else {
            self.vim.bare_exec("clap#state#clear_screen")
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
    Unactionable,

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
                Some(if provider_id == "blines" {
                    let mut index = 0;
                    lines_iter
                        .map(|line| {
                            let item: Arc<dyn ClapItem> = Arc::new(BlinesItem {
                                raw: line,
                                line_number: index + 1,
                            });
                            index += 1;
                            MatchedItem::from(item)
                        })
                        .collect()
                } else {
                    lines_iter
                        .map(|line| MatchedItem::from(Arc::new(line) as Arc<dyn ClapItem>))
                        .collect()
                })
            }
            _ => None,
        }
    }
}

/// A trait each Clap provider must implement.
#[async_trait::async_trait]
pub trait ClapProvider: Debug + Send + Sync + 'static {
    async fn on_initialize(&mut self, ctx: &mut Context) -> Result<()> {
        initialize_provider(ctx).await
    }

    async fn on_move(&mut self, ctx: &mut Context) -> Result<()> {
        if !ctx.env.preview_enabled {
            return Ok(());
        }
        ctx.update_preview().await
    }

    async fn on_typed(&mut self, ctx: &mut Context) -> Result<()>;

    /// On receiving the Terminate event.
    ///
    /// Sets the running signal to false, in case of the forerunner thread is still working.
    fn on_terminate(&mut self, ctx: &mut Context, session_id: u64) {
        ctx.signify_terminated(session_id);
    }

    async fn on_key_event(&mut self, ctx: &mut Context, key_event: KeyEvent) -> Result<()> {
        match key_event {
            KeyEvent::ShiftUp => {
                // Preview scroll up
            }
            KeyEvent::ShiftDown => {
                // Preview scroll down
            }
            KeyEvent::CtrlN => ctx.next_input().await?,
            KeyEvent::CtrlP => ctx.previous_input().await?,
            _ => {}
        }
        Ok(())
    }
}
