use std::collections::HashMap;

use anyhow::Result;
use crossbeam_channel::Sender;
use log::error;

use crate::stdio_server::{session::SessionId, types::MethodCall, SessionEvent};

/// A small wrapper of Sender<SessionEvent> for logging on send error.
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
        if let Err(e) = self.sender.send(event) {
            error!("Failed to send session event, error: {:?}", e);
        }
    }
}

/// Creates a new session with a context built from the message `msg`.
pub trait NewSession {
    /// Spawns a new session thread given `msg`.
    fn spawn(msg: MethodCall) -> Result<Sender<SessionEvent>>;
}

/// This structs manages all the created sessions tracked by the session id.
#[derive(Debug, Default)]
pub struct SessionManager {
    sessions: HashMap<SessionId, SessionEventSender>,
}

impl SessionManager {
    /// Starts a session in a new thread given the init message.
    pub fn new_session<T: NewSession>(&mut self, msg: MethodCall) {
        let session_id = msg.session_id;
        if self.exists(session_id) {
            error!("Skipped as session {} already exists", msg.session_id);
        } else {
            match T::spawn(msg) {
                Ok(sender) => {
                    sender
                        .send(SessionEvent::Create)
                        .expect("Failed to send Create Event");
                    self.sessions
                        .insert(session_id, SessionEventSender::new(sender, session_id));
                }
                Err(e) => {
                    error!("Could not spawn new session, error:{:?}", e);
                }
            }
        }
    }

    /// Returns true if the session exists given `session_id`.
    pub fn exists(&self, session_id: SessionId) -> bool {
        self.sessions.contains_key(&session_id)
    }

    /// Stop the session thread by sending [`SessionEvent::Terminate`].
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
                "Couldn't find `session_id`: {}, current available sessions: {:?}",
                session_id,
                self.sessions.keys()
            );
        }
    }
}
