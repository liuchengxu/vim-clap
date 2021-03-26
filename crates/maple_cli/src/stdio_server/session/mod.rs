mod context;
pub mod handlers;
mod manager;
mod providers;

use anyhow::Result;

use super::*;
use crate::stdio_server::types::ProviderId;

pub use self::context::SessionContext;
pub use self::handlers::on_move::{as_absolute_path, build_abs_path, OnMove, OnMoveHandler};
pub use self::manager::{Manager, NewSession};
pub use self::providers::*;

pub type SessionId = u64;

pub enum RpcMessage {
    OnMove(Message),
    OnTyped(Message),
}

pub trait HandleMessage: Send + 'static {
    fn handle(&self, msg: RpcMessage, context: &SessionContext);
}

#[derive(Debug, Clone)]
pub struct Session<T> {
    pub session_id: u64,
    pub context: SessionContext,
    /// Each Session can have its own message processing logic.
    pub message_handler: T,
    pub event_recv: crossbeam_channel::Receiver<SessionEvent>,
}

#[derive(Debug, Clone)]
pub enum SessionEvent {
    OnTyped(Message),
    OnMove(Message),
    Terminate,
}

impl<T: HandleMessage> Session<T> {
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
                        debug!("event(in) receive a session event: {:?}", event);
                        match event {
                            SessionEvent::Terminate => {
                                self.handle_terminate();
                                return;
                            }
                            SessionEvent::OnMove(msg) => self
                                .message_handler
                                .handle(RpcMessage::OnMove(msg), &self.context),
                            SessionEvent::OnTyped(msg) => self
                                .message_handler
                                .handle(RpcMessage::OnTyped(msg), &self.context),
                        }
                    }
                    Err(err) => debug!("session recv error: {:?}", err),
                }
            }
        });

        Ok(())
    }
}
