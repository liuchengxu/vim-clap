mod context;
mod manager;

use std::borrow::Cow;
use std::collections::HashSet;
use std::sync::Arc;

use anyhow::Result;
use crossbeam_channel::Sender;
use once_cell::sync::Lazy;
use parking_lot::Mutex;

use crate::stdio_server::providers::builtin::on_session_create;
use crate::stdio_server::{rpc::Call, types::ProviderId, MethodCall};

pub use self::context::{Scale, SessionContext, SyncFilterResults};
pub use self::manager::{NewSession, SessionManager};

static BACKGROUND_JOBS: Lazy<Arc<Mutex<HashSet<u64>>>> =
    Lazy::new(|| Arc::new(Mutex::new(HashSet::default())));

pub type SessionId = u64;

#[async_trait::async_trait]
pub trait EventHandler: Send + Sync + 'static {
    async fn handle_on_move(&mut self, msg: MethodCall, context: Arc<SessionContext>)
        -> Result<()>;
    async fn handle_on_typed(
        &mut self,
        msg: MethodCall,
        context: Arc<SessionContext>,
    ) -> Result<()>;
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
    Create,
    Terminate,
}

impl SessionEvent {
    /// Simplified display of session event.
    pub fn short_display(&self) -> Cow<'_, str> {
        match self {
            Self::OnTyped(msg) => format!("OnTyped, msg_id: {}", msg.id).into(),
            Self::OnMove(msg) => format!("OnMove, msg_id: {}", msg.id).into(),
            Self::Create => "Create".into(),
            Self::Terminate => "Terminate".into(),
        }
    }
}

impl<T: EventHandler> Session<T> {
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

    fn process_source_scale(&self, scale: Scale) {
        if let Some(total) = scale.total() {
            let method = "s:set_total_size";
            utility::println_json_with_length!(total, method);
        }

        if let Some(lines) = scale.initial_lines(100) {
            printer::decorate_lines::<i64>(
                lines,
                self.context.display_winwidth as usize,
                self.context.icon,
            )
            .print_on_session_create();
        }

        let mut val = self.context.scale.lock();
        *val = scale;
    }

    async fn handle_create(&mut self) {
        let context_clone = self.context.clone();

        const TIMEOUT: u64 = 300;

        match tokio::time::timeout(
            std::time::Duration::from_millis(TIMEOUT),
            on_session_create(context_clone),
        )
        .await
        {
            Ok(scale_result) => match scale_result {
                Ok(scale) => self.process_source_scale(scale),
                Err(e) => tracing::error!(?e, "Error occurred on creating session"),
            },
            Err(_) => {
                tracing::debug!(timeout = TIMEOUT, "Did not receive value in time");
                match self.context.provider_id.as_str() {
                    "grep" | "grep2" => {
                        let rg_cmd = crate::command::grep::RgBaseCommand::new(
                            self.context.cwd.to_path_buf(),
                        );
                        let job_id = utility::calculate_hash(&rg_cmd.inner);
                        let mut background_jobs = BACKGROUND_JOBS.lock();
                        if background_jobs.contains(&job_id) {
                            tracing::debug!(job_id, "An existing job for grep/grep2");
                        } else {
                            tracing::debug!(job_id, "Spawning a background job for grep/grep2");
                            background_jobs.insert(job_id);

                            tokio::spawn(async move {
                                let res = rg_cmd.create_cache().await;
                                let mut background_jobs = BACKGROUND_JOBS.lock();
                                background_jobs.remove(&job_id);
                                tracing::debug!(
                                  job_id,
                                  result = ?res,
                                  "The background job is done",
                                );
                            });
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    async fn process_event(&mut self, event: SessionEvent) -> Result<()> {
        match event {
            SessionEvent::Terminate => self.handle_terminate(),
            SessionEvent::Create => self.handle_create().await,
            SessionEvent::OnMove(msg) => {
                self.event_handler
                    .handle_on_move(msg, self.context.clone())
                    .await?;
            }
            SessionEvent::OnTyped(msg) => {
                // TODO: use a buffered channel here, do not process on every
                // single char change.
                self.event_handler
                    .handle_on_typed(msg, self.context.clone())
                    .await?;
            }
        }
        Ok(())
    }

    pub fn start_event_loop(mut self) {
        tokio::spawn(async move {
            tracing::debug!(
                session_id = self.session_id,
                provider_id = %self.provider_id(),
                "Spawning a new session task",
            );
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
        });
    }
}
