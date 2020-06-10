use super::*;
use crate::types::Message;
use crossbeam_channel::Sender;
use log::error;
use std::collections::HashMap;

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

fn spawn_new_session(msg: Message) -> anyhow::Result<Sender<SessionEvent>> {
    let (session_sender, session_receiver) = crossbeam_channel::unbounded();
    let msg_id = msg.id;

    let session = Session {
        session_id: msg.session_id,
        context: msg.into(),
        event_recv: session_receiver,
    };

    if session.context.source_cmd.is_some() {
        let session_cloned = session.clone();
        // TODO: choose different fitler strategy according to the time forerunner job spent.
        spawn_forerunner(msg_id, session_cloned)?;
    }

    session.start_event_loop()?;

    Ok(session_sender)
}

#[derive(Debug, Default)]
pub struct SessionManager {
    sessions: HashMap<SessionId, SessionEventSender>,
}

impl SessionManager {
    /// Start a session in a new thread given the session id and init message.
    pub fn new_session(&mut self, session_id: SessionId, msg: Message) {
        if self.has(session_id) {
            error!("Session {} already exists", msg.session_id);
        } else {
            match spawn_new_session(msg) {
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
