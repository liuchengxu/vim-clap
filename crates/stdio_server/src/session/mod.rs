mod context;
mod forerunner;
mod handler;
mod manager;

use super::*;
use crate::types::ProviderId;
use anyhow::Result;

pub use context::SessionContext;
pub use handler::{
    on_move::{as_absolute_path, build_abs_path, OnMove, OnMoveHandler},
    HandleMessage, RpcMessage,
};
pub use manager::{Manager, NewSession, OpaqueSession};

pub type SessionId = u64;

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

impl<T: handler::HandleMessage> Session<T> {
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
        thread::Builder::new()
            .name(format!(
                "session-{}-{}",
                self.session_id,
                self.provider_id()
            ))
            .spawn(move || loop {
                match self.event_recv.recv() {
                    Ok(event) => {
                        debug!("session recv: {:?}", event);
                        match event {
                            SessionEvent::Terminate => {
                                self.handle_terminate();
                                return;
                            }
                            SessionEvent::OnMove(msg) => self
                                .message_handler
                                .handle(handler::RpcMessage::OnMove(msg), &self.context),
                            SessionEvent::OnTyped(msg) => self
                                .message_handler
                                .handle(handler::RpcMessage::OnTyped(msg), &self.context),
                        }
                    }
                    Err(err) => debug!("session recv error: {:?}", err),
                }
            })?;
        Ok(())
    }
}
