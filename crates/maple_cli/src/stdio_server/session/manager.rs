use std::collections::HashMap;

use tokio::sync::mpsc::UnboundedSender;

use super::ClapProvider;
use crate::stdio_server::rpc::Call;
use crate::stdio_server::session::{ProviderEvent, Session, SessionId};

/// A small wrapper of Sender<ProviderEvent> for logging on sending error.
#[derive(Debug)]
pub struct ProviderEventSender {
    pub sender: UnboundedSender<ProviderEvent>,
    pub id: SessionId,
}

impl ProviderEventSender {
    pub fn new(sender: UnboundedSender<ProviderEvent>, id: SessionId) -> Self {
        Self { sender, id }
    }
}

impl std::fmt::Display for ProviderEventSender {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ProviderEventSender for session {}", self.id)
    }
}

impl ProviderEventSender {
    pub fn send(&self, event: ProviderEvent) {
        if let Err(error) = self.sender.send(event) {
            tracing::error!(?error, "Failed to send session event");
        }
    }
}

/// This structs manages all the created sessions tracked by the session id.
#[derive(Debug, Default)]
pub struct SessionManager {
    sessions: HashMap<SessionId, ProviderEventSender>,
}

impl SessionManager {
    /// Starts a session in a background task.
    pub fn new_session(&mut self, init_call: Call, provider_handle: Box<dyn ClapProvider>) {
        let session_id = init_call.session_id();

        if self.exists(session_id) {
            tracing::error!(session_id, "Skipped as given session already exists");
        } else {
            let (session, session_sender) = Session::new(session_id, provider_handle);
            session.start_event_loop();

            session_sender
                .send(ProviderEvent::Create(init_call))
                .expect("Failed to send Create Event");

            self.sessions.insert(
                session_id,
                ProviderEventSender::new(session_sender, session_id),
            );
        }
    }

    /// Returns true if the session exists given `session_id`.
    pub fn exists(&self, session_id: SessionId) -> bool {
        self.sessions.contains_key(&session_id)
    }

    /// Stop the session task by sending [`ProviderEvent::Terminate`].
    pub fn terminate(&mut self, session_id: SessionId) {
        if let Some(sender) = self.sessions.remove(&session_id) {
            sender.send(ProviderEvent::Terminate);
        }
    }

    /// Dispatch the session event to the background session task accordingly.
    pub fn send(&self, session_id: SessionId, event: ProviderEvent) {
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
