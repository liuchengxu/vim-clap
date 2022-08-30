use std::fmt::Debug;
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use printer::DisplayLines;
use serde_json::json;
use tokio::sync::mpsc::UnboundedSender;

use types::{ClapItem, MatchedItem};

use crate::stdio_server::impls::initialize_provider_source;
use crate::stdio_server::job;
use crate::stdio_server::session::{SessionContext, SessionId};
use crate::stdio_server::vim::Vim;

/// This type represents the scale of filtering source.
#[derive(Debug, Clone)]
pub enum ProviderSource {
    /// The provider source is unknown, probably a provider whose source is a List or a function
    /// returning a List.
    Unknown,

    /// Shell command to generate the provider source.
    #[allow(unused)]
    Command(String),

    // TODO: Use Arc<dyn ClapItem> instead of String.
    /// Small scale, in which case we do not have to use the dynamic filtering.
    Small {
        total: usize,
        items: Vec<Arc<dyn ClapItem>>,
    },

    /// Unknown scale, but the cache exists.
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

#[derive(Debug, Clone)]
pub enum ProviderEvent {
    Create,
    OnMove,
    OnTyped,
    // TODO: OnTab for filer
    Terminate,
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
    fn vim(&self) -> &Vim;

    fn session_context(&self) -> &SessionContext;

    async fn on_create(&mut self) -> Result<()> {
        const TIMEOUT: Duration = Duration::from_millis(300);

        let context = self.session_context();
        let vim = self.vim();

        match tokio::time::timeout(TIMEOUT, initialize_provider_source(context, vim)).await {
            Ok(provider_source_result) => match provider_source_result {
                Ok(provider_source) => {
                    if let Some(total) = provider_source.total() {
                        self.vim().set_var("g:clap.display.initial_size", total)?;
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

                        self.vim().exec(
                            "clap#state#init_display",
                            json!({
                              "lines": lines,
                              "icon_added": icon_added,
                              "truncated_map": truncated_map,
                            }),
                        )?;
                    }

                    context.set_provider_source(provider_source);
                }
                Err(e) => tracing::error!(?e, "Error occurred on creating session"),
            },
            Err(_) => {
                // The initialization was not super fast.
                tracing::debug!(timeout = ?TIMEOUT, "Did not receive value in time");

                let source_cmd: Vec<String> = vim.call("provider_source_cmd", json!([])).await?;
                let maybe_source_cmd = source_cmd.into_iter().next();
                if let Some(source_cmd) = maybe_source_cmd {
                    context.set_provider_source(ProviderSource::Command(source_cmd));
                }

                // Try creating cache for some potential heavy providers.
                match context.provider_id.as_str() {
                    "grep" | "grep2" => {
                        let rg_cmd =
                            crate::command::grep::RgTokioCommand::new(context.cwd.to_path_buf());
                        let job_id = utility::calculate_hash(&rg_cmd);
                        job::try_start(
                            async move {
                                let _ = rg_cmd.create_cache().await;
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

    async fn on_tab(&mut self) -> Result<()> {
        // Most providers don't need this, hence a default impl is provided.
        Ok(())
    }

    /// Sets the running signal to false, in case of the forerunner thread is still working.
    fn handle_terminate(&self, session_id: u64) {
        let context = self.session_context();
        context.state.is_running.store(false, Ordering::SeqCst);
        tracing::debug!(
            session_id,
            provider_id = %context.provider_id,
            "Session terminated",
        );
    }
}
