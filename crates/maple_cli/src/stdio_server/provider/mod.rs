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
use crate::stdio_server::rpc::Params;
use crate::stdio_server::session::SessionId;
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
use tokio::sync::mpsc::UnboundedSender;
use types::{ClapItem, MatchedItem};

pub async fn create_provider(
    provider_id: &str,
    context: ProviderContext,
) -> Result<Box<dyn ClapProvider>> {
    let provider: Box<dyn ClapProvider> = match provider_id {
        "blines" => Box::new(blines::BlinesProvider::new(context)),
        "dumb_jump" => Box::new(dumb_jump::DumbJumpProvider::new(context)),
        "filer" => Box::new(filer::FilerProvider::new(context)),
        "files" => Box::new(files::FilesProvider::new(context).await?),
        "grep" => Box::new(grep::GrepProvider::new(context)),
        "recent_files" => Box::new(recent_files::RecentFilesProvider::new(context)),
        _ => Box::new(generic_provider::GenericProvider::new(context)),
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
pub struct ProviderContext {
    pub cwd: AbsPathBuf,
    pub env: Arc<ProviderEnvironment>,
    pub vim: Vim,
    pub terminated: Arc<AtomicBool>,
    pub preview_cache: Arc<RwLock<HashMap<PreviewTarget, Preview>>>,
    pub provider_source: Arc<RwLock<ProviderSource>>,
}

impl ProviderContext {
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
            env: Arc::new(env),
            vim,
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

#[derive(Debug, Clone, Copy)]
pub enum Key {
    Tab,
    Backspace,
    // <CR>/<Enter>/<Return> was typed.
    CarriageReturn,
    // <S-Up>
    ShiftUp,
    // <S-Down>
    ShiftDown,
}

#[derive(Debug, Clone)]
pub enum ProviderEvent {
    Create,
    OnMove,
    OnTyped,
    Terminate,
    KeyTyped(Key),
}

#[derive(Debug, Clone)]
pub enum Event {
    Provider(ProviderEvent),
    Other(String),
}

impl Event {
    pub fn from_method(method: &str) -> Self {
        match method {
            "new_session" => Self::Provider(ProviderEvent::Create),
            "on_typed" => Self::Provider(ProviderEvent::OnTyped),
            "on_move" => Self::Provider(ProviderEvent::OnMove),
            "exit" => Self::Provider(ProviderEvent::Terminate),
            "cr" => Self::Provider(ProviderEvent::KeyTyped(Key::CarriageReturn)),
            "tab" => Self::Provider(ProviderEvent::KeyTyped(Key::Tab)),
            "backspace" => Self::Provider(ProviderEvent::KeyTyped(Key::Backspace)),
            "shift-up" => Self::Provider(ProviderEvent::KeyTyped(Key::ShiftUp)),
            "shift-down" => Self::Provider(ProviderEvent::KeyTyped(Key::ShiftDown)),
            other => Self::Other(other.to_string()),
        }
    }
}

/// A small wrapper of Sender<ProviderEvent> for logging on sending error.
#[derive(Debug)]
pub struct ProviderEventSender {
    pub sender: UnboundedSender<ProviderEvent>,
    pub id: SessionId,
}

impl ProviderEventSender {
    pub fn new(sender: UnboundedSender<ProviderEvent>, id: SessionId) -> Self {
        Self { sender, id }
    }
}

impl std::fmt::Display for ProviderEventSender {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ProviderEventSender for session {}", self.id)
    }
}

impl ProviderEventSender {
    pub fn send(&self, event: ProviderEvent) {
        if let Err(error) = self.sender.send(event) {
            tracing::error!(?error, "Failed to send session event");
        }
    }
}

/// A trait each Clap provider must implement.
#[async_trait::async_trait]
pub trait ClapProvider: Debug + Send + Sync + 'static {
    fn context(&self) -> &ProviderContext;

    async fn on_create(&mut self) -> Result<()> {
        initialize_provider(self.context()).await
    }

    async fn on_move(&mut self) -> Result<()>;

    async fn on_typed(&mut self) -> Result<()>;

    async fn on_key_typed(&mut self, key: Key) -> Result<()> {
        match key {
            Key::ShiftUp => {
                // Preview scroll up
            }
            Key::ShiftDown => {
                // Preview scroll down
            }
            _ => {}
        }
        Ok(())
    }

    /// On receiving the Terminate event.
    ///
    /// Sets the running signal to false, in case of the forerunner thread is still working.
    fn on_terminate(&mut self, session_id: u64) {
        self.context().signify_terminated(session_id);
    }
}
