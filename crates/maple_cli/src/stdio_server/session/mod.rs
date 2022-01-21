mod context;
mod manager;

use std::borrow::Cow;
use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use crossbeam_channel::Sender;
use futures::Future;
use once_cell::sync::Lazy;
use parking_lot::Mutex;

use crate::stdio_server::providers::builtin::on_session_create;
use crate::stdio_server::{rpc::Call, types::ProviderId, MethodCall};

pub use self::context::{Scale, SessionContext, SyncFilterResults};
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

fn process_source_scale(scale: Scale, context: Arc<SessionContext>) {
    if let Some(total) = scale.total() {
        let method = "s:set_total_size";
        utility::println_json_with_length!(total, method);
    }

    if let Some(lines) = scale.initial_lines(100) {
        printer::decorate_lines::<i64>(lines, context.display_winwidth as usize, context.icon)
            .print_on_session_create();
    }

    let mut val = context.scale.lock();
    *val = scale;
}

#[async_trait::async_trait]
pub trait EventHandle: Send + Sync + 'static {
    async fn on_create(&mut self, _call: Call, context: Arc<SessionContext>) {
        const TIMEOUT: Duration = Duration::from_millis(300);

        match tokio::time::timeout(TIMEOUT, on_session_create(context.clone())).await {
            Ok(scale_result) => match scale_result {
                Ok(scale) => process_source_scale(scale, context),
                Err(e) => tracing::error!(?e, "Error occurred on creating session"),
            },
            Err(_) => {
                tracing::debug!(timeout = ?TIMEOUT, "Did not receive value in time");
                match context.provider_id.as_str() {
                    "grep" | "grep2" => {
                        let rg_cmd =
                            crate::command::grep::RgBaseCommand::new(context.cwd.to_path_buf());
                        let job_id = utility::calculate_hash(&rg_cmd.inner);
                        spawn_singleton_job(
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
    }

    async fn on_move(&mut self, msg: MethodCall, context: Arc<SessionContext>) -> Result<()>;

    async fn on_typed(&mut self, msg: MethodCall, context: Arc<SessionContext>) -> Result<()>;
}

#[derive(Debug, Clone)]
pub struct Session<T> {
    pub session_id: u64,
    pub context: Arc<SessionContext>,
    /// Each Session can have its own message processing logic.
    pub event_handler: T,
    pub event_recv: crossbeam_channel::Receiver<SessionEvent>,
    pub source_scale: Scale,
}

#[derive(Debug, Clone)]
pub enum SessionEvent {
    OnTyped(MethodCall),
    OnMove(MethodCall),
    Create(Call),
    Terminate,
}

impl SessionEvent {
    /// Simplified display of session event.
    pub fn short_display(&self) -> Cow<'_, str> {
        match self {
            Self::OnTyped(msg) => format!("OnTyped, msg_id: {}", msg.id).into(),
            Self::OnMove(msg) => format!("OnMove, msg_id: {}", msg.id).into(),
            Self::Create(_) => "Create".into(),
            Self::Terminate => "Terminate".into(),
        }
    }
}

impl<T: EventHandle> Session<T> {
    pub fn new(call: Call, event_handler: T) -> (Self, Sender<SessionEvent>) {
        let (session_sender, session_receiver) = crossbeam_channel::unbounded();

        let session = Session {
            session_id: call.session_id(),
            context: Arc::new(call.into()),
            event_handler,
            event_recv: session_receiver,
            source_scale: Scale::Indefinite,
        };

        (session, session_sender)
    }

    /// Sets the running signal to false, in case of the forerunner thread is still working.
    pub fn handle_terminate(&mut self) {
        let mut val = self.context.is_running.lock();
        *val.get_mut() = false;
        tracing::debug!(
            session_id = self.session_id,
            provider_id = %self.provider_id(),
            "Session terminated",
        );
    }

    pub fn provider_id(&self) -> &ProviderId {
        &self.context.provider_id
    }

    pub fn start_event_loop(mut self) {
        if self.context.debounce {
            tokio::spawn(async move {
                self.run_event_loop_with_debounce().await;
            });
        } else {
            tokio::spawn(async move {
                self.run_event_loop_without_debounce().await;
            });
        }
    }

    async fn process_event(&mut self, event: SessionEvent) -> Result<()> {
        match event {
            SessionEvent::Terminate => self.handle_terminate(),
            SessionEvent::Create(call) => {
                self.event_handler
                    .on_create(call, self.context.clone())
                    .await
            }
            SessionEvent::OnMove(msg) => {
                self.event_handler
                    .on_move(msg, self.context.clone())
                    .await?;
            }
            SessionEvent::OnTyped(msg) => {
                // TODO: use a buffered channel here, do not process on every
                // single char change.
                self.event_handler
                    .on_typed(msg, self.context.clone())
                    .await?;
            }
        }
        Ok(())
    }

    async fn run_event_loop_without_debounce(mut self) {
        loop {
            match self.event_recv.recv() {
                Ok(event) => {
                    tracing::debug!(event = ?event.short_display(), "Received an event");
                    if let Err(err) = self.process_event(event).await {
                        tracing::debug!(?err, "Error processing SessionEvent");
                    }
                }
                Err(err) => {
                    tracing::debug!(?err, "The channel is possibly broken");
                    break;
                }
            }
        }
    }

    async fn run_event_loop_with_debounce(mut self) {
        use crossbeam_channel::select;

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
            provider_id = %self.provider_id(),
            "Spawning a new session task",
        );

        let mut pending_on_typed = None;
        let mut debounce_timer = NEVER;

        loop {
            select! {
              recv(self.event_recv) -> maybe_event => {
                  match maybe_event {
                      Ok(event) => {
                          tracing::debug!(event = ?event.short_display(), "Received an event");
                          match event {
                              SessionEvent::Terminate => self.handle_terminate(),
                              SessionEvent::Create(call) => {
                                  self.event_handler
                                      .on_create(call, self.context.clone())
                                      .await
                              }
                              SessionEvent::OnMove(msg) => {
                                  if let Err(err) =
                                      self.event_handler.on_move(msg, self.context.clone()).await
                                  {
                                      tracing::error!(?err, "Error processing SessionEvent::OnMove");
                                  }
                              }
                              SessionEvent::OnTyped(msg) => {
                                  pending_on_typed.replace(msg);
                                  debounce_timer = DELAY;
                              }
                          }
                      }
                      Err(err) => {
                          tracing::debug!(?err, "The channel is possibly broken");
                          return;
                      }
                  }
              }
              default(debounce_timer) => {
                  debounce_timer = NEVER;
                  if let Some(msg) = pending_on_typed.take() {
                      if let Err(err) = self.event_handler.on_typed(msg, self.context.clone()).await {
                          tracing::error!(?err, "Error processing SessionEvent::OnTyped");
                      }
                  }
              }
            }
        }
    }
}
