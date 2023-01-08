mod blines;
mod dumb_jump;
mod filer;
mod files;
mod generic_provider;
mod grep;
mod recent_files;

pub use self::filer::read_dir_entries;
use crate::paths::AbsPathBuf;
use crate::stdio_server::handler::{initialize_provider, Preview, PreviewTarget};
use crate::stdio_server::input::KeyEvent;
use crate::stdio_server::rpc::Params;
use crate::stdio_server::vim::Vim;
use anyhow::Result;
use icon::{Icon, IconKind};
use matcher::{Bonus, MatchScope, MatcherBuilder};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
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
    pub preview: BufnrWinid,
    pub icon: Icon,
    pub matcher_builder: MatcherBuilder,
    pub debounce: bool,
    pub no_cache: bool,
    pub display_winwidth: usize,
    pub start_buffer_path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct Context {
    pub cwd: AbsPathBuf,
    pub vim: Vim,
    pub env: Arc<ProviderEnvironment>,
    pub terminated: Arc<AtomicBool>,
    pub preview_cache: Arc<RwLock<HashMap<PreviewTarget, Preview>>>,
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
            preview: BufnrWinid,
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
            preview,
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
        let matcher_builder = provider_id.matcher_builder();
        let display_winwidth = vim.winwidth(display.winid).await?;
        let is_nvim: usize = vim.call("has", ["nvim"]).await?;

        let env = ProviderEnvironment {
            is_nvim: is_nvim == 1,
            provider_id,
            start,
            input,
            display,
            preview,
            no_cache,
            debounce,
            start_buffer_path,
            display_winwidth,
            matcher_builder,
            icon,
        };

        Ok(Self {
            cwd,
            vim,
            env: Arc::new(env),
            terminated: Arc::new(AtomicBool::new(false)),
            preview_cache: Arc::new(RwLock::new(HashMap::new())),
            provider_source: Arc::new(RwLock::new(ProviderSource::Unactionable)),
        })
    }

    pub fn provider_id(&self) -> &str {
        self.env.provider_id.as_str()
    }

    /// Executes the command `cmd` and returns the raw bytes of stdout.
    pub fn execute(&self, cmd: &str) -> std::io::Result<Vec<u8>> {
        let out = utility::execute_at(cmd, Some(&self.cwd))?;
        Ok(out.stdout)
    }

    pub fn set_provider_source(&self, new: ProviderSource) {
        let mut provider_source = self.provider_source.write();
        *provider_source = new;
    }

    pub async fn preview_height(&self) -> Result<usize> {
        self.vim
            .preview_size(&self.env.provider_id, self.env.preview.winid)
            .await
            .map(|x| 2 * x)
    }

    pub fn start_buffer_extension(&self) -> Result<String> {
        self.env
            .start_buffer_path
            .extension()
            .and_then(|s| s.to_str())
            .map(|s| s.to_string())
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "Extension not found for start_buffer_path: {}",
                    self.env.start_buffer_path.display()
                )
            })
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

    pub fn signify_terminated(&self, session_id: u64) {
        self.terminated.store(true, Ordering::SeqCst);
        tracing::debug!("Session {session_id:?}-{} terminated", self.provider_id(),);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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

    pub fn initial_items(&self, n: usize) -> Option<Vec<MatchedItem>> {
        match self {
            Self::Small { ref items, .. } => Some(
                items
                    .iter()
                    .take(n)
                    .map(|item| MatchedItem::from(item.clone()))
                    .collect(),
            ),
            Self::File { ref path, .. } | Self::CachedFile { ref path, .. } => Some(
                utility::read_first_lines(path, n)
                    .ok()?
                    .map(|line| MatchedItem::from(Arc::new(line) as Arc<dyn ClapItem>))
                    .collect(),
            ),
            _ => None,
        }
    }
}

/// A trait each Clap provider must implement.
#[async_trait::async_trait]
pub trait ClapProvider: Debug + Send + Sync + 'static {
    async fn on_create(&mut self, ctx: &mut Context) -> Result<()> {
        initialize_provider(ctx).await
    }

    async fn on_move(&mut self, ctx: &mut Context) -> Result<()>;

    async fn on_typed(&mut self, ctx: &mut Context) -> Result<()>;

    /// On receiving the Terminate event.
    ///
    /// Sets the running signal to false, in case of the forerunner thread is still working.
    fn on_terminate(&mut self, ctx: &mut Context, session_id: u64) {
        ctx.signify_terminated(session_id);
    }

    async fn on_key_event(&mut self, _ctx: &mut Context, key_event: KeyEvent) -> Result<()> {
        match key_event {
            KeyEvent::ShiftUp => {
                // Preview scroll up
            }
            KeyEvent::ShiftDown => {
                // Preview scroll down
            }
            _ => {}
        }
        Ok(())
    }
}
