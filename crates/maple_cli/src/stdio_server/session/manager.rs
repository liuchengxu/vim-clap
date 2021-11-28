use std::collections::HashMap;

use anyhow::Result;
use crossbeam_channel::Sender;

use crate::stdio_server::{rpc::Call, session::SessionId, MethodCall, SessionEvent};

/// A small wrapper of Sender<SessionEvent> for logging on sending error.
#[derive(Debug)]
pub struct SessionEventSender {
    pub sender: Sender<SessionEvent>,
    pub id: SessionId,
}

impl SessionEventSender {
    pub fn new(sender: Sender<SessionEvent>, id: SessionId) -> Self {
        Self { sender, id }
    }
}

impl std::fmt::Display for SessionEventSender {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SessionEventSender for session {}", self.id)
    }
}

impl SessionEventSender {
    pub fn send(&self, event: SessionEvent) {
        if let Err(error) = self.sender.send(event) {
            tracing::error!(?error, "Failed to send session event");
        }
    }
}

/// Creates a new session with a context built from the message `msg`.
pub trait NewSession {
    /// Spawns a new session thread given `msg`.
    fn spawn(call: Call) -> Result<Sender<SessionEvent>>;
}

/// This structs manages all the created sessions tracked by the session id.
#[derive(Debug, Default)]
pub struct SessionManager {
    sessions: HashMap<SessionId, SessionEventSender>,
}

impl SessionManager {
    /// Starts a session in a background task.
    pub fn new_session<T: NewSession>(&mut self, call: Call) {
        let session_id = call.session_id();
        if self.exists(session_id) {
            tracing::error!(session_id, "Skipped as given session already exists");
        } else {
            match T::spawn(call) {
                Ok(sender) => {
                    sender
                        .send(SessionEvent::Create)
                        .expect("Failed to send Create Event");
                    self.sessions
                        .insert(session_id, SessionEventSender::new(sender, session_id));
                }
                Err(error) => {
                    tracing::error!(?error, "Failed not spawn a new session");
                }
            }
        }
    }

    /// Returns true if the session exists given `session_id`.
    pub fn exists(&self, session_id: SessionId) -> bool {
        self.sessions.contains_key(&session_id)
    }

    /// Stop the session task by sending [`SessionEvent::Terminate`].
    pub fn terminate(&mut self, session_id: SessionId) {
        if let Some(sender) = self.sessions.remove(&session_id) {
            sender.send(SessionEvent::Terminate);
        }
    }

    /// Dispatch the session event to the background session task accordingly.
    pub fn send(&self, session_id: SessionId, event: SessionEvent) {
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
