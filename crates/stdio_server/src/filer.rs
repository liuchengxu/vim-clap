use super::{write_response, Message};
use crate::session::{
    build_abs_path, HandleMessage, NewSession, OnMove, OnMoveHandler, RpcMessage, Session,
    SessionContext, SessionEvent,
};
use anyhow::Result;
use crossbeam_channel::Sender;
use icon::prepend_filer_icon;
use log::debug;
use serde_json::json;
use std::path::{self, Path, PathBuf};
use std::{fs, io};

/// Display the inner path in a nicer way.
struct DisplayPath {
    inner: PathBuf,
    enable_icon: bool,
}

impl DisplayPath {
    pub fn new(path: PathBuf, enable_icon: bool) -> Self {
        Self {
            inner: path,
            enable_icon,
        }
    }

    #[inline]
    fn to_file_name_str(&self) -> Option<&str> {
        self.inner.file_name().and_then(std::ffi::OsStr::to_str)
    }
}

impl Into<String> for DisplayPath {
    fn into(self) -> String {
        let path_str = if self.inner.is_dir() {
            format!(
                "{}{}",
                self.to_file_name_str().unwrap(),
                path::MAIN_SEPARATOR
            )
        } else {
            self.to_file_name_str().map(Into::into).unwrap()
        };

        if self.enable_icon {
            prepend_filer_icon(&self.inner, &path_str)
        } else {
            path_str
        }
    }
}

pub fn read_dir_entries<P: AsRef<Path>>(
    dir: P,
    enable_icon: bool,
    max: Option<usize>,
) -> Result<Vec<String>> {
    let entries_iter =
        fs::read_dir(dir)?.map(|res| res.map(|x| DisplayPath::new(x.path(), enable_icon).into()));

    let mut entries = if let Some(m) = max {
        entries_iter
            .take(m)
            .collect::<Result<Vec<_>, io::Error>>()?
    } else {
        entries_iter.collect::<Result<Vec<_>, io::Error>>()?
    };

    entries.sort();

    Ok(entries)
}

pub struct FilerSession;

#[derive(Clone)]
pub struct FilerMessageHandler;

impl HandleMessage for FilerMessageHandler {
    fn handle(&self, msg: RpcMessage, context: &SessionContext) {
        match msg {
            RpcMessage::OnMove(msg) => {
                let provider_id = context.provider_id.clone();
                let curline = msg.get_curline(&provider_id).unwrap();
                let path = build_abs_path(&msg.get_cwd(), curline);
                let on_move_handler = OnMoveHandler {
                    msg_id: msg.id,
                    size: provider_id.get_preview_size(),
                    provider_id,
                    context,
                    inner: OnMove::Filer(path.clone()),
                };
                if let Err(err) = on_move_handler.handle() {
                    let error = json!({"message": format!("{}", err), "dir": path});
                    let res = json!({ "id": msg.id, "provider_id": "filer", "error": error });
                    write_response(res);
                }
            }
            // TODO: handle on_typed
            RpcMessage::OnTyped(msg) => handle_message(msg),
        }
    }
}

impl NewSession for FilerSession {
    fn spawn(&self, msg: Message) -> Result<Sender<SessionEvent>> {
        let (session_sender, session_receiver) = crossbeam_channel::unbounded();

        let session = Session {
            session_id: msg.session_id,
            context: msg.clone().into(),
            message_handler: FilerMessageHandler,
            event_recv: session_receiver,
        };

        // handle on_init
        handle_message(msg);

        session.start_event_loop()?;

        Ok(session_sender)
    }
}

pub(super) fn handle_message(msg: Message) {
    let cwd = msg.get_cwd();
    debug!("Recv filer params: cwd:{}", cwd,);

    let result = match read_dir_entries(&cwd, crate::env::global().enable_icon, None) {
        Ok(entries) => {
            let result = json!({
            "entries": entries,
            "dir": cwd,
            "total": entries.len(),
            });
            json!({ "id": msg.id, "provider_id": "filer", "result": result })
        }
        Err(err) => {
            let error = json!({"message": format!("{}", err), "dir": cwd});
            json!({ "id": msg.id, "provider_id": "filer", "error": error })
        }
    };

    write_response(result);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dir() {
        let entries = read_dir_entries(
            &std::env::current_dir()
                .unwrap()
                .into_os_string()
                .into_string()
                .unwrap(),
            false,
            None,
        )
        .unwrap();
        println!("entry: {:?}", entries);
    }
}
