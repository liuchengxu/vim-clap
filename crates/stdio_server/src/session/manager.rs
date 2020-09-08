use super::*;
use crate::types::Message;
use anyhow::Result;
use crossbeam_channel::Sender;
use log::error;
use std::collections::HashMap;

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

#[derive(Debug, Default)]
pub struct Manager {
    sessions: HashMap<SessionId, SessionEventSender>,
}

pub trait NewSession {
    fn spawn(&self, msg: Message) -> Result<Sender<SessionEvent>>;
}

pub struct OpaqueSession;

impl NewSession for OpaqueSession {
    fn spawn(&self, msg: Message) -> Result<Sender<SessionEvent>> {
        let (session_sender, session_receiver) = crossbeam_channel::unbounded();
        let msg_id = msg.id;

        let session = Session {
            session_id: msg.session_id,
            context: msg.clone().into(),
            message_handler: super::handler::MessageHandler,
            event_recv: session_receiver,
        };

        if let Some(source_cmd) = session.context.source_cmd.clone() {
            let session_cloned = session.clone();
            // TODO: choose different fitler strategy according to the time forerunner job spent.
            thread::Builder::new()
                .name(format!("session-forerunner-{}", session.session_id))
                .spawn(move || {
                    crate::session::forerunner::run(msg_id, source_cmd, session_cloned)
                })?;
        }

        session.start_event_loop()?;

        Ok(session_sender)
    }
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

    pub fn new_opaque_session(&mut self, session_id: SessionId, msg: Message) {
        self.new_session(session_id, msg, OpaqueSession)
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
