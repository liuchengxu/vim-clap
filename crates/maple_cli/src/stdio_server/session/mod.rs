mod context;
mod manager;

use std::collections::HashSet;
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

use crate::stdio_server::impls::initialize;
use crate::stdio_server::vim::Vim;

pub use self::context::{SessionContext, SourceScale};
pub use self::manager::SessionManager;

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

pub type SessionId = u64;

#[async_trait::async_trait]
pub trait ClapProvider: Debug + Send + Sync + 'static {
    fn vim(&self) -> &Vim;

    fn session_context(&self) -> &SessionContext;

    async fn on_create(&mut self) -> Result<()> {
        const TIMEOUT: Duration = Duration::from_millis(300);

        let context = self.session_context();

        // TODO: blocking on_create for the swift providers like `tags`.
        match tokio::time::timeout(TIMEOUT, initialize(context)).await {
            Ok(scale_result) => match scale_result {
                Ok(scale) => {
                    if let Some(total) = scale.total() {
                        self.vim()
                            .exec("set_var", json!(["g:clap.display.initial_size", total]))?;
                    }

                    if let Some(lines) = scale.initial_lines(100) {
                        let DisplayLines {
                            lines,
                            truncated_map,
                            icon_added,
                            ..
                        } = printer::decorate_lines(
                            lines,
                            context.display_winwidth as usize,
                            context.icon,
                        );

                        self.vim().exec("clap#state#init_display", json!({ "lines": lines, "truncated_map": truncated_map, "icon_added": icon_added }))?;
                    }

                    context.set_source_scale(scale);
                }
                Err(e) => tracing::error!(?e, "Error occurred on creating session"),
            },
            Err(_) => {
                // The initialization was not super fast.
                tracing::debug!(timeout = ?TIMEOUT, "Did not receive value in time");

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
    pub session_id: u64,
    /// Each provider session can have its own message processing logic.
    pub provider: Box<dyn ClapProvider>,
    pub event_recv: tokio::sync::mpsc::UnboundedReceiver<ProviderEvent>,
}

#[derive(Debug, Clone)]
pub enum ProviderEvent {
    OnTyped,
    Create,
    OnMove,
    Terminate,
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

        tracing::debug!(
            session_id = self.session_id,
            provider_id = %self.provider.session_context().provider_id,
            "Spawning a new session task",
        );

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
                                ProviderEvent::Terminate => self.provider.handle_terminate(self.session_id),
                                ProviderEvent::Create => {
                                  tracing::debug!("============================= Processing Create");
                                    if let Err(err) = self.provider.on_create().await {
                                        tracing::error!(?err, "Error processing ProviderEvent::Create");
                                    }
                                  tracing::debug!("============================= Processing Create Done!");
                                }
                                ProviderEvent::OnMove => {
                                    tracing::debug!("============================= Processing OnMove");
                                    if let Err(err) = self.provider.on_move().await {
                                        tracing::error!(?err, "Error processing ProviderEvent::OnMove");
                                    }
                                    tracing::debug!("============================= Processing OnMove Done!");
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
                        tracing::error!(?err, "Error processing ProviderEvent::OnTyped");
                    }
                }
            }
        }
    }

    async fn run_event_loop_without_debounce(mut self) {
        while let Some(event) = self.event_recv.recv().await {
            tracing::debug!(?event, "[without_debounce] Received an event");

            match event {
                ProviderEvent::Terminate => self.provider.handle_terminate(self.session_id),
                ProviderEvent::Create => {
                    if let Err(err) = self.provider.on_create().await {
                        tracing::error!(?err, "Error processing ProviderEvent::Create");
                    }
                }
                ProviderEvent::OnMove => {
                    if let Err(err) = self.provider.on_move().await {
                        tracing::debug!(?err, "Error processing ProviderEvent::OnMove");
                    }
                }
                ProviderEvent::OnTyped => {
                    if let Err(err) = self.provider.on_typed().await {
                        tracing::debug!(?err, "Error processing ProviderEvent::OnTyped");
                    }
                }
            }
        }
    }
}
