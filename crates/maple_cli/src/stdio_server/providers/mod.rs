pub mod dumb_jump;
pub mod filer;
pub mod quickfix;
pub mod recent_files;

use anyhow::Result;
use crossbeam_channel::Sender;

use crate::stdio_server::event_handlers::{self, DefaultEventHandler};
use crate::stdio_server::{
    session::{NewSession, Session, SessionEvent},
    Message,
};

pub struct GeneralSession;

impl NewSession for GeneralSession {
    fn spawn(msg: Message) -> Result<Sender<SessionEvent>> {
        let msg_id = msg.id;

        let (session, session_sender) = Session::new(msg, DefaultEventHandler);
        log::debug!("New general session context: {:?}", session.context);

        // FIXME: Actually unused for now
        if let Some(source_cmd) = session.context.source_cmd.clone() {
            let session_cloned = session.clone();
            // TODO: choose different fitler strategy according to the time forerunner job spent.
            tokio::spawn(async move {
                if let Err(e) =
                    event_handlers::on_init::run(msg_id, source_cmd, session_cloned).await
                {
                    log::error!(
                        "Error occurred when running the forerunner job, msg_id: {}, error: {:?}",
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
