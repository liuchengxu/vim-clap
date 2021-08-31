mod context;
mod manager;

use std::sync::Arc;

use anyhow::Result;
use crossbeam_channel::Sender;
use log::debug;

use crate::stdio_server::providers::builtin::on_session_create;
use crate::stdio_server::types::{Message, ProviderId};

pub use self::context::{Scale, SessionContext, SyncFilterResults};
pub use self::manager::{NewSession, SessionManager};

pub type SessionId = u64;

#[async_trait::async_trait]
pub trait EventHandler: Send + Sync + 'static {
    async fn handle_on_move(&mut self, msg: Message, context: Arc<SessionContext>) -> Result<()>;
    async fn handle_on_typed(&mut self, msg: Message, context: Arc<SessionContext>) -> Result<()>;
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
    OnTyped(Message),
    OnMove(Message),
    Create,
    Terminate,
}

impl SessionEvent {
    /// Simplified display of session event.
    pub fn short_display(&self) -> String {
        match self {
            Self::OnTyped(msg) => format!("OnTyped, msg id: {}", msg.id),
            Self::OnMove(msg) => format!("OnMove, msg id: {}", msg.id),
            Self::Create => "Create".into(),
            Self::Terminate => "Terminate".into(),
        }
    }
}

impl<T: EventHandler> Session<T> {
    pub fn new(msg: Message, event_handler: T) -> (Self, Sender<SessionEvent>) {
        let (session_sender, session_receiver) = crossbeam_channel::unbounded();

        let session = Session {
            session_id: msg.session_id,
            context: Arc::new(msg.into()),
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
        debug!(
            "session-{}-{} terminated",
            self.session_id,
            self.provider_id()
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

        if let Scale::Small { ref lines, .. } = scale {
            printer::decorate_lines::<i64>(
                lines.iter().take(200).map(|s| s.as_str().into()).collect(),
                self.context.display_winwidth as usize,
                self.context.icon.clone().into(),
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
                Err(e) => log::error!("Error occurred on session create: {:?}", e),
            },
            Err(_) => log::debug!("Did not receive value with {} ms", TIMEOUT),
        }
    }

    pub fn start_event_loop(mut self) {
        tokio::spawn(async move {
            debug!(
                "Spawning a new task for session-{}-{}",
                self.session_id,
                self.provider_id()
            );
            loop {
                match self.event_recv.recv() {
                    Ok(event) => {
                        debug!("Received an event: {}", event.short_display());
                        match event {
                            SessionEvent::Terminate => {
                                self.handle_terminate();
                                return;
                            }
                            SessionEvent::Create => self.handle_create().await,
                            SessionEvent::OnMove(msg) => {
                                if let Err(e) = self
                                    .event_handler
                                    .handle_on_move(msg, self.context.clone())
                                    .await
                                {
                                    debug!("Error occurrred when handling OnMove event: {:?}", e);
                                }
                            }
                            SessionEvent::OnTyped(msg) => {
                                if let Err(e) = self
                                    .event_handler
                                    .handle_on_typed(msg, self.context.clone())
                                    .await
                                {
                                    debug!("Error occurrred when handling OnTyped event: {:?}", e);
                                }
                            }
                        }
                    }
                    Err(err) => {
                        debug!("The channel is possibly broken, error: {:?}", err);
                        break;
                    }
                }
            }
        });
    }
}
