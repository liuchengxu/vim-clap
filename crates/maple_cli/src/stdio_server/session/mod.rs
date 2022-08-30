mod context;

use std::collections::{HashMap, HashSet};
use std::fmt::Debug;
use std::sync::{atomic::Ordering, Arc};
use std::time::Duration;

use anyhow::Result;
use futures::Future;
use once_cell::sync::Lazy;
use parking_lot::Mutex;
use printer::DisplayLines;
use serde_json::json;
use tokio::sync::mpsc::UnboundedSender;
use tokio::time::Instant;

use crate::stdio_server::impls::initialize_provider_source;
use crate::stdio_server::vim::Vim;

pub use self::context::{ProviderSource, SessionContext};

pub type SessionId = u64;

static BACKGROUND_JOBS: Lazy<Arc<Mutex<HashSet<u64>>>> =
    Lazy::new(|| Arc::new(Mutex::new(HashSet::default())));

pub fn spawn_singleton_job(
    task_future: impl Future<Output = ()> + Send + Sync + 'static,
    job_id: u64,
) {
    if register_job_successfully(job_id) {
        tokio::spawn(async move {
            task_future.await;
            note_job_is_finished(job_id)
        });
    }
}

pub fn register_job_successfully(job_id: u64) -> bool {
    let mut background_jobs = BACKGROUND_JOBS.lock();
    if background_jobs.contains(&job_id) {
        false
    } else {
        background_jobs.insert(job_id);
        true
    }
}

pub fn note_job_is_finished(job_id: u64) {
    let mut background_jobs = BACKGROUND_JOBS.lock();
    background_jobs.remove(&job_id);
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

        // TODO: blocking on_create for the swift providers like `tags`.
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
                        spawn_singleton_job(
                            async move {
                                let _ = rg_cmd.create_cache().await;
                            },
                            job_id,
                        );
                    }
                    _ => {
                        // TODO: Note arbitrary shell command and use par_dyn_run later.
                    }
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
        //
        // Add extra 50ms delay.
        const DELAY: Duration = Duration::from_millis(200 + 50);
        // If the debounce timer isn't active, it will be set to expire "never",
        // which is actually just 1 year in the future.
        const NEVER: Duration = Duration::from_secs(365 * 24 * 60 * 60);

        let mut dirty = false;

        let debounce_timer = tokio::time::sleep(NEVER);
        tokio::pin!(debounce_timer);

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
                                    if let Err(err) = self.provider.on_create().await {
                                        tracing::error!(?err, "Failed to process {event:?}");
                                    }
                                }
                                ProviderEvent::OnMove => {
                                    if let Err(err) = self.provider.on_move().await {
                                        tracing::error!(?err, "Failed to process {event:?}");
                                    }
                                }
                                ProviderEvent::OnTyped => {
                                    dirty = true;
                                    debounce_timer.as_mut().reset(Instant::now() + DELAY);
                                }
                            }
                          }
                          None => break, // channel has closed.
                      }
                }
                _ = debounce_timer.as_mut(), if dirty => {
                    dirty = false;
                    debounce_timer.as_mut().reset(Instant::now() + NEVER);

                    if let Err(err) = self.provider.on_typed().await {
                        tracing::error!(?err, "Failed to process ProviderEvent::OnTyped");
                    }
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
    /// Starts a session in a background task.
    pub fn new_session(&mut self, session_id: SessionId, provider: Box<dyn ClapProvider>) {
        if self.exists(session_id) {
            tracing::error!(session_id, "Skipped as given session already exists");
        } else {
            let (session, session_sender) = Session::new(session_id, provider);
            session.start_event_loop();

            session_sender
                .send(ProviderEvent::Create)
                .expect("Failed to send Create Event");

            self.sessions.insert(
                session_id,
                ProviderEventSender::new(session_sender, session_id),
            );
        }
    }

    /// Returns true if the session exists given `session_id`.
    pub fn exists(&self, session_id: SessionId) -> bool {
        self.sessions.contains_key(&session_id)
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
