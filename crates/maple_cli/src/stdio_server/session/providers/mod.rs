pub mod dumb_jump;
pub mod filer;

use anyhow::Result;
use crossbeam_channel::Sender;
use log::debug;

use crate::stdio_server::{
    session::{
        event_handlers::{self, DefaultEventHandler},
        NewSession, Session, SessionEvent,
    },
    Message,
};

pub struct GeneralSession;

impl NewSession for GeneralSession {
    fn spawn(&self, msg: Message) -> Result<Sender<SessionEvent>> {
        let (session_sender, session_receiver) = crossbeam_channel::unbounded();
        let msg_id = msg.id;

        let session = Session {
            session_id: msg.session_id,
            context: msg.into(),
            event_handler: DefaultEventHandler,
            event_recv: session_receiver,
        };

        debug!("new session context: {:?}", session.context);

        // FIXME: Actually unused for now
        if let Some(source_cmd) = session.context.source_cmd.clone() {
            let session_cloned = session.clone();
            // TODO: choose different fitler strategy according to the time forerunner job spent.
            tokio::spawn(async move {
                if let Err(e) =
                    event_handlers::on_init::run(msg_id, source_cmd, session_cloned).await
                {
                    log::error!(
                        "error occurred when running the forerunner job, msg_id: {}, error: {:?}",
                        msg_id,
                        e
                    );
                }
            });
        }

        session.start_event_loop()?;

        Ok(session_sender)
    }
}
