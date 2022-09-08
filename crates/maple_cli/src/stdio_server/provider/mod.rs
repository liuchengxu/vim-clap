mod default_impl;
mod dumb_jump;
mod filer;
mod recent_files;

use std::fmt::Debug;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use parking_lot::RwLock;
use printer::DisplayLines;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::sync::mpsc::UnboundedSender;

use icon::{Icon, IconKind};
use matcher::{Bonus, FuzzyAlgorithm, MatchScope, Matcher};
use types::{ClapItem, MatchedItem};

use crate::stdio_server::handler::initialize_provider_source;
use crate::stdio_server::job;
use crate::stdio_server::rpc::Params;
use crate::stdio_server::session::SessionId;
use crate::stdio_server::vim::Vim;

pub use self::default_impl::DefaultProvider;
pub use self::dumb_jump::DumbJumpProvider;
pub use self::filer::{read_dir_entries, FilerProvider};
pub use self::recent_files::RecentFilesProvider;

/// bufnr and winid.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BufnrWinid {
    pub bufnr: usize,
    pub winid: usize,
}

#[derive(Debug, Clone)]
pub struct ProviderContext {
    pub provider_id: ProviderId,
    pub start: BufnrWinid,
    pub input: BufnrWinid,
    pub display: BufnrWinid,
    pub preview: BufnrWinid,
    pub cwd: PathBuf,
    pub icon: Icon,
    pub matcher: Matcher,
    pub no_cache: bool,
    pub debounce: bool,
    pub display_winwidth: usize,
    pub start_buffer_path: PathBuf,
    pub vim: Vim,
    pub terminated: Arc<AtomicBool>,
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
            cwd: PathBuf,
            icon: String,
            no_cache: bool,
            debounce: bool,
            start_buffer_path: PathBuf,
        }

        let InnerParams {
            provider_id,
            start,
            input,
            display,
            preview,
            cwd,
            no_cache,
            debounce,
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

        let matcher = provider_id.matcher();

        let display_winwidth = vim.winwidth(display.winid).await?;

        Ok(Self {
            provider_id,
            start,
            input,
            display,
            preview,
            cwd,
            no_cache,
            debounce,
            start_buffer_path,
            display_winwidth,
            matcher,
            icon,
            vim,
            terminated: Arc::new(AtomicBool::new(false)),
            provider_source: Arc::new(RwLock::new(ProviderSource::Unknown)),
        })
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderId(String);

impl ProviderId {
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Returns the preview size of current provider.
    #[inline]
    pub fn get_preview_size(&self) -> usize {
        super::global().preview_size_of(&self.0)
    }

    pub fn matcher(&self) -> Matcher {
        let match_scope = match self.0.as_str() {
            "grep" | "grep2" => MatchScope::GrepLine,
            "tags" | "proj_tags" => MatchScope::TagName,
            _ => MatchScope::Full,
        };

        let match_bonuses = match self.0.as_str() {
            "files" | "git_files" | "filer" => vec![Bonus::FileName],
            _ => vec![],
        };

        Matcher::with_bonuses(match_bonuses, FuzzyAlgorithm::Fzy, match_scope)
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

/// This type represents the scale of filtering source.
#[derive(Debug, Clone)]
pub enum ProviderSource {
    /// The provider source is unknown.
    Unknown,

    /// Shell command to generate the provider source.
    Command(String),

    /// Small scale, in which case we do not have to use the dynamic filtering.
    ///
    /// The scale can be small for some known swift providers or when a provider's source
    /// is a List or a function returning a List.
    Small {
        total: usize,
        items: Vec<Arc<dyn ClapItem>>,
    },

    /// Cache file exists, reuse the cache instead of executing the command again.
    CachedFile { total: usize, path: PathBuf },
}

impl Default for ProviderSource {
    fn default() -> Self {
        Self::Unknown
    }
}

impl ProviderSource {
    pub fn total(&self) -> Option<usize> {
        match self {
            Self::Small { total, .. } | Self::CachedFile { total, .. } => Some(*total),
            _ => None,
        }
    }

    pub fn initial_lines(&self, n: usize) -> Option<Vec<MatchedItem>> {
        match self {
            Self::Small { ref items, .. } => Some(
                items
                    .iter()
                    .take(n)
                    .map(|item| {
                        MatchedItem::new(item.clone(), Default::default(), Default::default())
                    })
                    .collect(),
            ),
            Self::CachedFile { ref path, .. } => utility::read_first_lines(path, n)
                .map(|iter| {
                    iter.map(|line| {
                        MatchedItem::new(Arc::new(line), Default::default(), Default::default())
                    })
                    .collect()
                })
                .ok(),
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
}

#[derive(Debug, Clone)]
pub enum ProviderEvent {
    Create,
    OnMove,
    OnTyped,
    Terminate,
    KeyTyped(Key),
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

/// A trait that each Clap provider should implement.
#[async_trait::async_trait]
pub trait ClapProvider: Debug + Send + Sync + 'static {
    fn context(&self) -> &ProviderContext;

    async fn on_create(&mut self) -> Result<()> {
        const TIMEOUT: Duration = Duration::from_millis(300);

        let context = self.context();

        match tokio::time::timeout(TIMEOUT, initialize_provider_source(context)).await {
            Ok(provider_source_result) => match provider_source_result {
                Ok(provider_source) => {
                    if let Some(total) = provider_source.total() {
                        self.context()
                            .vim
                            .set_var("g:clap.display.initial_size", total)?;
                    }
                    if let Some(lines) = provider_source.initial_lines(100) {
                        let DisplayLines {
                            lines,
                            icon_added,
                            truncated_map,
                            ..
                        } = printer::decorate_lines(
                            lines,
                            context.display_winwidth as usize,
                            context.icon,
                        );

                        context.vim.exec(
                            "clap#state#init_display",
                            json!([lines, truncated_map, icon_added]),
                        )?;
                    }

                    context.set_provider_source(provider_source);
                }
                Err(e) => tracing::error!(?e, "Error occurred on creating session"),
            },
            Err(_) => {
                // The initialization was not super fast.
                tracing::debug!(timeout = ?TIMEOUT, "Did not receive value in time");

                let source_cmd: Vec<String> =
                    context.vim.call("provider_source_cmd", json!([])).await?;
                let maybe_source_cmd = source_cmd.into_iter().next();
                if let Some(source_cmd) = maybe_source_cmd {
                    context.set_provider_source(ProviderSource::Command(source_cmd));
                }

                // Try creating cache for some potential heavy providers.
                match context.provider_id.as_str() {
                    "grep" | "grep2" => {
                        context.set_provider_source(ProviderSource::Command(
                            crate::command::grep::RG_EXEC_CMD.to_string(),
                        ));

                        let context = context.clone();
                        let rg_cmd = crate::command::grep::RgTokioCommand::new(context.cwd.clone());
                        let job_id = utility::calculate_hash(&rg_cmd);
                        job::try_start(
                            async move {
                                if let Ok(digest) = rg_cmd.create_cache().await {
                                    let new = ProviderSource::CachedFile {
                                        total: digest.total,
                                        path: digest.cached_path,
                                    };
                                    if !context.terminated.load(Ordering::SeqCst) {
                                        context.set_provider_source(new);
                                    }
                                }
                            },
                            job_id,
                        );
                    }
                    _ => {}
                }
            }
        }

        Ok(())
    }

    async fn on_move(&mut self) -> Result<()>;

    async fn on_typed(&mut self) -> Result<()>;

    async fn on_key_typed(&mut self, _key: Key) -> Result<()> {
        // Most providers don't need this, hence a default impl is provided.
        Ok(())
    }

    /// Sets the running signal to false, in case of the forerunner thread is still working.
    fn handle_terminate(&mut self, session_id: u64) {
        self.context().terminated.store(true, Ordering::SeqCst);
        tracing::debug!(
            session_id,
            provider_id = %self.context().provider_id,
            "Session terminated",
        );
    }
}
