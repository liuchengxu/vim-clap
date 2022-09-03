//! Each invocation of Clap provider is a session. When you exit the provider, the session ends.

use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::fmt::Debug;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::UnboundedSender;
use tokio::time::Instant;

use icon::{Icon, IconKind};
use matcher::Matcher;

use crate::stdio_server::provider::{
    ClapProvider, ProviderEvent, ProviderEventSender, ProviderId, ProviderSource,
};
use crate::stdio_server::rpc::Params;
use crate::stdio_server::vim::Vim;

pub type SessionId = u64;

/// bufnr and winid.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BufnrWinid {
    pub bufnr: usize,
    pub winid: usize,
}

#[derive(Debug, Clone)]
pub struct SessionContext {
    pub provider_id: ProviderId,
    pub start: BufnrWinid,
    pub input: BufnrWinid,
    pub display: BufnrWinid,
    pub preview: BufnrWinid,
    pub cwd: PathBuf,
    pub no_cache: bool,
    pub debounce: bool,
    pub start_buffer_path: PathBuf,
    pub display_winwidth: usize,
    pub icon: Icon,
    pub matcher: Matcher,
    pub provider_source: Arc<RwLock<ProviderSource>>,
}

impl SessionContext {
    pub async fn new(params: Params, vim: &Vim) -> Result<Self> {
        #[derive(Deserialize)]
        struct InnerParams {
            provider_id: ProviderId,
            cwd: PathBuf,
            icon: String,
            debounce: bool,
            no_cache: bool,
            start: BufnrWinid,
            input: BufnrWinid,
            display: BufnrWinid,
            preview: BufnrWinid,
            source_fpath: PathBuf,
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
            source_fpath,
            icon,
        } = params
            .parse()
            .expect("Failed to deserialize SessionContext");

        let icon = match icon.to_lowercase().as_str() {
            "buffertags" => Icon::Enabled(IconKind::BufferTags),
            "projtags" => Icon::Enabled(IconKind::ProjTags),
            "file" => Icon::Enabled(IconKind::File),
            "grep" => Icon::Enabled(IconKind::Grep),
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
            start_buffer_path: source_fpath,
            display_winwidth,
            matcher,
            icon,
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

#[derive(Debug)]
pub struct Session {
    session_id: u64,
    /// Each provider session can have its own message processing logic.
    provider: Box<dyn ClapProvider>,
    event_recv: tokio::sync::mpsc::UnboundedReceiver<ProviderEvent>,
}

impl Session {
    pub fn new(
        session_id: u64,
        provider: Box<dyn ClapProvider>,
    ) -> (Self, UnboundedSender<ProviderEvent>) {
        let (session_sender, session_receiver) = tokio::sync::mpsc::unbounded_channel();

        let session = Session {
            session_id,
            provider,
            event_recv: session_receiver,
        };

        (session, session_sender)
    }

    pub fn start_event_loop(self) {
        tracing::debug!(
            session_id = self.session_id,
            provider_id = %self.provider.session_context().provider_id,
            "Spawning a new session event loop task",
        );

        tokio::spawn(async move {
            if self.provider.session_context().debounce {
                self.run_event_loop_with_debounce().await;
            } else {
                self.run_event_loop_without_debounce().await;
            }
        });
    }

    async fn run_event_loop_with_debounce(mut self) {
        // https://github.com/denoland/deno/blob/1fb5858009f598ce3f917f9f49c466db81f4d9b0/cli/lsp/diagnostics.rs#L141
        //
        // Debounce timer delay. 150ms between keystrokes is about 45 WPM, so we
        // want something that is longer than that, but not too long to
        // introduce detectable UI delay; 200ms is a decent compromise.
        const DELAY: Duration = Duration::from_millis(200);
        // If the debounce timer isn't active, it will be set to expire "never",
        // which is actually just 1 year in the future.
        const NEVER: Duration = Duration::from_secs(365 * 24 * 60 * 60);

        let mut on_move_dirty = false;
        let on_move_delay = Duration::from_millis(50);
        let on_move_timer = tokio::time::sleep(NEVER);
        tokio::pin!(on_move_timer);

        let mut on_typed_dirty = false;
        // Delay can be adjusted once we know the provider source scale.
        //
        // Here is the benchmark result of filtering on AMD 5900X:
        //
        // |    Type     |  1k   |  10k   | 100k  |
        // |    ----     |  ---- | ----   | ----  |
        // |     filter  | 413us | 12ms   | 75ms  |
        // | par_filter  | 327us |  3ms   | 20ms  |
        let mut on_typed_delay = DELAY;
        let on_typed_timer = tokio::time::sleep(NEVER);
        tokio::pin!(on_typed_timer);

        loop {
            tokio::select! {
                maybe_event = self.event_recv.recv() => {
                    match maybe_event {
                        Some(event) => {
                            tracing::debug!(?event, "[with_debounce] Received an event");

                            match event {
                                ProviderEvent::Terminate => {
                                    self.provider.handle_terminate(self.session_id);
                                    break;
                                }
                                ProviderEvent::Create => {
                                    match self.provider.on_create().await {
                                        Ok(()) => {
                                            if let ProviderSource::Small { total, .. } = *self
                                                .provider
                                                .session_context()
                                                .provider_source
                                                .read()
                                            {
                                                if total < 10_000 {
                                                    on_typed_delay = Duration::from_millis(10);
                                                } else if total < 100_000 {
                                                    on_typed_delay = Duration::from_millis(50);
                                                } else if total < 200_000 {
                                                    on_typed_delay = Duration::from_millis(100);
                                                }
                                            }
                                        }
                                        Err(err) => {
                                            tracing::error!(?err, "Failed to process {event:?}");
                                        }
                                    }
                                }
                                ProviderEvent::OnMove => {
                                    on_move_dirty = true;
                                    on_move_timer.as_mut().reset(Instant::now() + on_move_delay);
                                }
                                ProviderEvent::OnTyped => {
                                    on_typed_dirty = true;
                                    on_typed_timer.as_mut().reset(Instant::now() + on_typed_delay);
                                }
                            }
                          }
                          None => break, // channel has closed.
                      }
                }
                _ = on_move_timer.as_mut(), if on_move_dirty => {
                    on_move_dirty = false;
                    on_move_timer.as_mut().reset(Instant::now() + NEVER);

                    if let Err(err) = self.provider.on_move().await {
                        tracing::error!(?err, "Failed to process ProviderEvent::OnMove");
                    }
                }
                _ = on_typed_timer.as_mut(), if on_typed_dirty => {
                    on_typed_dirty = false;
                    on_typed_timer.as_mut().reset(Instant::now() + NEVER);

                    if let Err(err) = self.provider.on_typed().await {
                        tracing::error!(?err, "Failed to process ProviderEvent::OnTyped");
                    }

                    let _ = self.provider.on_move().await;
                }
            }
        }
    }

    async fn run_event_loop_without_debounce(mut self) {
        while let Some(event) = self.event_recv.recv().await {
            tracing::debug!(?event, "[without_debounce] Received an event");

            match event {
                ProviderEvent::Terminate => {
                    self.provider.handle_terminate(self.session_id);
                    break;
                }
                ProviderEvent::Create => {
                    if let Err(err) = self.provider.on_create().await {
                        tracing::error!(?err, "Failed at process {event:?}");
                    }
                }
                ProviderEvent::OnMove => {
                    if let Err(err) = self.provider.on_move().await {
                        tracing::debug!(?err, "Failed to process {event:?}");
                    }
                }
                ProviderEvent::OnTyped => {
                    if let Err(err) = self.provider.on_typed().await {
                        tracing::debug!(?err, "Failed to process {event:?}");
                    }
                }
            }
        }
    }
}

/// This structs manages all the created sessions tracked by the session id.
#[derive(Debug, Default)]
pub struct SessionManager {
    sessions: HashMap<SessionId, ProviderEventSender>,
}

impl SessionManager {
    /// Creates a new session if session_id does not exist.
    pub fn new_session(&mut self, session_id: SessionId, provider: Box<dyn ClapProvider>) {
        if let Entry::Vacant(v) = self.sessions.entry(session_id) {
            let (session, session_sender) = Session::new(session_id, provider);
            session.start_event_loop();

            session_sender
                .send(ProviderEvent::Create)
                .expect("Failed to send Create Event");

            v.insert(ProviderEventSender::new(session_sender, session_id));
        } else {
            tracing::error!(session_id, "Skipped as given session already exists");
        }
    }

    /// Stop the session task by sending [`ProviderEvent::Terminate`].
    pub fn terminate(&mut self, session_id: SessionId) {
        if let Some(sender) = self.sessions.remove(&session_id) {
            sender.send(ProviderEvent::Terminate);
        }
    }

    /// Dispatch the session event to the background session task accordingly.
    pub fn send(&self, session_id: SessionId, event: ProviderEvent) {
        if let Some(sender) = self.sessions.get(&session_id) {
            sender.send(event);
        } else {
            tracing::error!(
                session_id,
                sessions = ?self.sessions.keys(),
                "Couldn't find the sender for given session",
            );
        }
    }
}
