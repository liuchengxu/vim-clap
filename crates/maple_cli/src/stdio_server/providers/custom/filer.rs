use std::path::{self, Path};
use std::sync::Arc;
use std::{fs, io};

use anyhow::Result;
use crossbeam_channel::Sender;
use jsonrpc_core::Value;
use serde_json::json;

use icon::prepend_filer_icon;

use crate::stdio_server::providers::builtin::{OnMove, OnMoveHandler};
use crate::stdio_server::{
    rpc::Call,
    session::{EventHandler, NewSession, Session, SessionContext, SessionEvent},
    write_response, MethodCall,
};
use crate::utils::build_abs_path;

/// Display the inner path in a nicer way.
struct DisplayPath<P> {
    inner: P,
    enable_icon: bool,
}

impl<P: AsRef<Path>> DisplayPath<P> {
    pub fn new(inner: P, enable_icon: bool) -> Self {
        Self { inner, enable_icon }
    }

    #[inline]
    fn as_file_name(&self) -> Option<&str> {
        self.inner
            .as_ref()
            .file_name()
            .and_then(std::ffi::OsStr::to_str)
    }
}

impl<P: AsRef<Path>> std::fmt::Display for DisplayPath<P> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let mut write_with_icon = |path: &str| {
            if self.enable_icon {
                write!(f, "{}", prepend_filer_icon(self.inner.as_ref(), path))
            } else {
                write!(f, "{}", path)
            }
        };

        if self.inner.as_ref().is_dir() {
            let path = format!("{}{}", self.as_file_name().unwrap(), path::MAIN_SEPARATOR);
            write_with_icon(&path)
        } else {
            write_with_icon(self.as_file_name().unwrap())
        }
    }
}

pub fn read_dir_entries<P: AsRef<Path>>(
    dir: P,
    enable_icon: bool,
    max: Option<usize>,
) -> Result<Vec<String>> {
    let entries_iter = fs::read_dir(dir)?
        .map(|res| res.map(|x| DisplayPath::new(x.path(), enable_icon).to_string()));

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

#[derive(Clone)]
pub struct FilerMessageHandler;

#[async_trait::async_trait]
impl EventHandler for FilerMessageHandler {
    async fn handle_on_move(
        &mut self,
        msg: MethodCall,
        context: Arc<SessionContext>,
    ) -> Result<()> {
        #[derive(serde::Deserialize)]
        struct Params {
            // curline: String,
            cwd: String,
        }
        let msg_id = msg.id;
        // Do not use curline directly.
        let curline = msg.get_curline(&context.provider_id)?;
        let Params { cwd } = msg.parse_unsafe();
        let path = build_abs_path(&cwd, curline);
        let on_move_handler = OnMoveHandler {
            msg_id,
            size: context.sensible_preview_size(),
            context: &context,
            inner: OnMove::Filer(path.clone()),
            expected_line: None,
        };
        if let Err(err) = on_move_handler.handle() {
            tracing::error!(?err, ?path, "Failed to handle filer OnMove");
            let res = json!({
              "id": msg_id,
              "provider_id": "filer",
              "error": { "message": err.to_string(), "dir": path }
            });
            write_response(res);
        }
        Ok(())
    }

    async fn handle_on_typed(
        &mut self,
        msg: MethodCall,
        _context: Arc<SessionContext>,
    ) -> Result<()> {
        handle_filer_message(msg);
        Ok(())
    }
}

pub struct FilerSession;

impl NewSession for FilerSession {
    fn spawn(call: Call) -> Result<Sender<SessionEvent>> {
        let (session, session_sender) = Session::new(call.clone(), FilerMessageHandler);

        // Handle the on_init message.
        handle_filer_message(call.unwrap_method_call());

        session.start_event_loop();

        Ok(session_sender)
    }
}

pub fn handle_filer_message(msg: MethodCall) -> std::result::Result<Value, Value> {
    let cwd = msg.get_cwd();
    tracing::debug!(?cwd, "Recv filer params");

    read_dir_entries(&cwd, crate::stdio_server::global().enable_icon, None)
        .map(|entries| {
            let result = json!({
            "entries": entries,
            "dir": cwd,
            "total": entries.len(),
            });
            json!({ "id": msg.id, "provider_id": "filer", "result": result })
        })
        .map_err(|err| {
            tracing::error!(?cwd, "Failed to read directory entries");
            let error = json!({"message": err.to_string(), "dir": cwd});
            json!({ "id": msg.id, "provider_id": "filer", "error": error })
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dir() {
        // /home/xlc/.vim/plugged/vim-clap/crates/stdio_server
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

        assert_eq!(entries, vec!["Cargo.toml", "benches/", "src/"]);
    }
}
