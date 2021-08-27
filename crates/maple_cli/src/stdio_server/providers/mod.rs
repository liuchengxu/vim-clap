// pub mod blines;
pub mod dumb_jump;
pub mod filer;
pub mod quickfix;
pub mod recent_files;

use anyhow::Result;
use crossbeam_channel::Sender;

use crate::stdio_server::event_handlers::DefaultEventHandler;
use crate::stdio_server::{
    session::{NewSession, Session, SessionEvent},
    Message,
};

pub struct GeneralSession;

impl NewSession for GeneralSession {
    fn spawn(msg: Message) -> Result<Sender<SessionEvent>> {
        let (session, session_sender) = Session::new(msg, DefaultEventHandler);
        session.start_event_loop()?;
        Ok(session_sender)
    }
}
