use std::collections::HashMap;

use anyhow::Result;
use crossbeam_channel::Sender;
use log::error;

use crate::stdio_server::{session::SessionId, types::Message, SessionEvent};

/// A small wrapper of Sender<SessionEvent> for logging on send error.
#[derive(Debug)]
pub struct SessionEventSender(Sender<SessionEvent>);

impl From<Sender<SessionEvent>> for SessionEventSender {
    fn from(sender: Sender<SessionEvent>) -> Self {
        Self(sender)
    }
}

impl SessionEventSender {
    pub fn send(&self, event: SessionEvent) {
        if let Err(e) = self.0.send(event) {
            error!("Failed to send session event, error: {:?}", e);
        }
    }
}

/// This structs manages all the created sessions tracked by the session id.
#[derive(Debug, Default)]
pub struct Manager {
    sessions: HashMap<SessionId, SessionEventSender>,
}

/// Creates a new session with a context built from the message `msg`.
pub trait NewSession {
    fn spawn(&self, msg: Message) -> Result<Sender<SessionEvent>>;
}

/// Dispatches the raw RpcMessage to the right session instance according to the session_id.
impl Manager {
    /// Starts a session in a new thread given the session id and init message.
    pub fn new_session<T: NewSession>(
        &mut self,
        session_id: SessionId,
        msg: Message,
        new_session: T,
    ) {
        if self.has(session_id) {
            error!("Session {} already exists", msg.session_id);
        } else {
            match new_session.spawn(msg) {
                Ok(sender) => {
                    self.sessions.insert(session_id, sender.into());
                }
                Err(e) => {
                    error!("Couldn't spawn new session, error:{:?}", e);
                }
            }
        }
    }

    /// Returns true if the sessoion exists given the session_id.
    pub fn has(&self, session_id: SessionId) -> bool {
        self.sessions.contains_key(&session_id)
    }

    /// Send Terminate event to stop the thread of session.
    pub fn terminate(&mut self, session_id: SessionId) {
        if let Some(sender) = self.sessions.remove(&session_id) {
            sender.send(SessionEvent::Terminate);
        }
    }

    /// Dispatch the session event to the session thread accordingly.
    pub fn send(&self, session_id: SessionId, event: SessionEvent) {
        if let Some(sender) = self.sessions.get(&session_id) {
            sender.send(event);
        } else {
            error!(
                "Can't find session_id: {} in SessionManager: {:?}",
                session_id, self
            );
        }
    }
}
