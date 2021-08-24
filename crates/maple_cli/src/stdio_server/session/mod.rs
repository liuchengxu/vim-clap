mod context;
mod manager;

use anyhow::Result;
use crossbeam_channel::Sender;
use log::debug;

use crate::stdio_server::types::{Message, ProviderId};

pub use self::context::SessionContext;
pub use self::manager::{NewSession, SessionManager};

pub type SessionId = u64;

#[derive(Debug)]
pub enum Event {
    OnMove(Message),
    OnTyped(Message),
}

#[async_trait::async_trait]
pub trait EventHandler: Send + Sync + 'static {
    /// Use the mutable self so that we can cache some info inside the handler.
    async fn handle(&mut self, event: Event, context: SessionContext) -> Result<()>;
}

#[derive(Debug, Clone)]
pub struct Session<T> {
    pub session_id: u64,
    pub context: SessionContext,
    /// Each Session can have its own message processing logic.
    pub event_handler: T,
    pub event_recv: crossbeam_channel::Receiver<SessionEvent>,
}

#[derive(Debug, Clone)]
pub enum SessionEvent {
    OnTyped(Message),
    OnMove(Message),
    Terminate,
}

impl SessionEvent {
    pub fn short_display(&self) -> String {
        match self {
            Self::OnTyped(msg) => format!("OnTyped, msg id: {}", msg.id),
            Self::OnMove(msg) => format!("OnMove, msg id: {}", msg.id),
            Self::Terminate => "Terminate".into(),
        }
    }
}

impl<T: EventHandler> Session<T> {
    pub fn new(msg: Message, event_handler: T) -> (Self, Sender<SessionEvent>) {
        let (session_sender, session_receiver) = crossbeam_channel::unbounded();

        let session = Session {
            session_id: msg.session_id,
            context: msg.into(),
            event_handler,
            event_recv: session_receiver,
        };

        (session, session_sender)
    }

    /// Sets the running signal to false, in case of the forerunner thread is still working.
    pub fn handle_terminate(&mut self) {
        let mut val = self.context.is_running.lock().unwrap();
        *val.get_mut() = false;
        debug!(
            "session-{}-{} terminated",
            self.session_id,
            self.provider_id()
        );
    }

    /// This session is still running, hasn't received Terminate event.
    pub fn is_running(&self) -> bool {
        self.context
            .is_running
            .lock()
            .unwrap()
            .load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Saves the forerunner result.
    /// TODO: Store full lines, or a cached file?
    pub fn set_source_list(&mut self, lines: Vec<String>) {
        let mut source_list = self.context.source_list.lock().unwrap();
        *source_list = Some(lines);
    }

    pub fn provider_id(&self) -> &ProviderId {
        &self.context.provider_id
    }

    pub fn start_event_loop(mut self) -> Result<()> {
        tokio::spawn(async move {
            debug!(
                "spawn a new task for session-{}-{}",
                self.session_id,
                self.provider_id()
            );
            loop {
                match self.event_recv.recv() {
                    Ok(event) => {
                        debug!(
                            "Event(in) receive a session event: {:?}",
                            event.short_display()
                        );
                        match event {
                            SessionEvent::Terminate => {
                                self.handle_terminate();
                                return;
                            }
                            SessionEvent::OnMove(msg) => {
                                // TODO: use Arc::new(self.context)
                                if let Err(e) = self
                                    .event_handler
                                    .handle(Event::OnMove(msg), self.context.clone())
                                    .await
                                {
                                    debug!("Error occurrred when handling OnMove event: {:?}", e);
                                }
                            }
                            SessionEvent::OnTyped(msg) => {
                                if let Err(e) = self
                                    .event_handler
                                    .handle(Event::OnTyped(msg), self.context.clone())
                                    .await
                                {
                                    debug!("Error occurrred when handling OnTyped event: {:?}", e);
                                }
                            }
                        }
                    }
                    Err(err) => debug!(
                        "The channel is possibly disconnected, session recv error: {:?}",
                        err
                    ),
                }
            }
        });

        Ok(())
    }
}
