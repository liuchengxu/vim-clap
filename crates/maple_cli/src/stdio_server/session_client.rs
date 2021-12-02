use std::sync::Arc;

use anyhow::Result;
use crossbeam_channel::Receiver;
use parking_lot::Mutex;
use serde_json::{json, Value};

use crate::stdio_server::rpc::{Call, MethodCall};
use crate::stdio_server::state::State;

use super::session::SessionManager;

#[derive(Clone)]
pub struct SessionClient {
    pub state_mutex: Arc<Mutex<State>>,
    pub session_manager: Arc<Mutex<SessionManager>>,
}

impl SessionClient {
    /// Creates a new instnace of [`SessionClient`].
    pub fn new(state: State) -> Self {
        Self {
            state_mutex: Arc::new(Mutex::new(state)),
            session_manager: Arc::new(Mutex::new(SessionManager::default())),
        }
    }

    /// Entry of the bridge between Vim and Rust.
    pub fn loop_call(&self, rx: &Receiver<Call>) {
        for call in rx.iter() {
            let session_client = self.clone();
            tokio::spawn(async move {
                if let Err(e) = session_client.handle_vim_message(call).await {
                    tracing::error!(?e, "Error handling request");
                }
            });
        }
    }

    /// Handle the message actively initiated from Vim.
    async fn handle_vim_message(self, call: Call) -> Result<()> {
        match call {
            Call::Notification(notification) => {
                if let Err(e) = notification.process().await {
                    tracing::error!(?e, "Error when handling notification");
                }
            }
            Call::MethodCall(method_call) => {
                let id = method_call.id;
                let maybe_result = self.process_method_call(method_call).await?;
                // Send back the result of method call.
                if let Some(result) = maybe_result {
                    let state = self.state_mutex.lock();
                    state.vim.rpc_client.output(id, Ok(result))?;
                }
            }
        }
        Ok(())
    }

    /// Process the method call message from Vim.
    async fn process_method_call(&self, method_call: MethodCall) -> Result<Option<Value>> {
        use super::dumb_jump::DumbJumpSession;
        use super::recent_files::RecentFilesSession;
        use super::SessionEvent::*;

        let msg = method_call;

        if msg.method != "init_ext_map" {
            tracing::debug!(?msg, "==> stdio message(in)");
        }

        let value = match msg.method.as_str() {
            "init_ext_map" => Some(msg.parse_filetypedetect()),
            "preview/file" => Some(msg.preview_file().await?),
            "quickfix" => Some(msg.preview_quickfix().await?),

            /*
            "dumb_jump/on_init" => self.session_manager.new_session::<DumbJumpSession>(msg),
            "dumb_jump/on_typed" => self.session_manager.send(msg.session_id, OnTyped(msg)),
            "dumb_jump/on_move" => self.session_manager.send(msg.session_id, OnMove(msg)),

            "recent_files/on_init" => manager.new_session::<RecentFilesSession>(msg),
            "recent_files/on_typed" => manager.send(msg.session_id, OnTyped(msg)),
            "recent_files/on_move" => manager.send(msg.session_id, OnMove(msg)),

            "filer" => filer::handle_filer_message(msg),
            "filer/on_init" => manager.new_session::<FilerSession>(msg),
            "filer/on_move" => manager.send(msg.session_id, OnMove(msg)),

            "on_init" => manager.new_session::<BuiltinSession>(msg),
            "on_typed" => manager.send(msg.session_id, OnTyped(msg)),
            "on_move" => manager.send(msg.session_id, OnMove(msg)),
            "exit" => manager.terminate(msg.session_id),
            */
            _ => Some(json!({
                "error": format!("Unknown method call: {}", msg.method)
            })),
        };

        Ok(value)
    }
}
