use std::sync::Arc;

use anyhow::Result;
use parking_lot::Mutex;
use serde_json::{json, Value};
use tokio::sync::mpsc::UnboundedReceiver;

use super::session::SessionManager;
use super::Notification;
use crate::stdio_server::impls::dumb_jump::DumbJumpProvider;
use crate::stdio_server::impls::filer::FilerProvider;
use crate::stdio_server::impls::recent_files::RecentFilesProvider;
use crate::stdio_server::impls::DefaultProvider;
use crate::stdio_server::rpc::{Call, MethodCall};
use crate::stdio_server::session::SessionContext;
use crate::stdio_server::state::State;

#[derive(Clone)]
pub struct SessionClient {
    pub state_mutex: Arc<Mutex<State>>,
    pub session_manager_mutex: Arc<Mutex<SessionManager>>,
}

impl SessionClient {
    /// Creates a new instnace of [`SessionClient`].
    pub fn new(state: State) -> Self {
        Self {
            state_mutex: Arc::new(Mutex::new(state)),
            session_manager_mutex: Arc::new(Mutex::new(SessionManager::default())),
        }
    }

    /// Entry of the bridge between Vim and Rust.
    pub async fn loop_call(&self, mut rx: UnboundedReceiver<Call>) {
        while let Some(call) = rx.recv().await {
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
                if let Err(e) = self.process_notification(notification).await {
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

    /// Process the notification message from Vim.
    async fn process_notification(&self, notification: Notification) -> Result<()> {
        match notification.method.as_str() {
            "initialize_global_env" => notification.initialize_global_env(), // should be called only once.
            "note_recent_files" => notification.note_recent_file().await,
            "on_init" => {
                let mut session_manager = self.session_manager_mutex.lock();
                let call = Call::Notification(notification);
                let context: SessionContext = call.clone().into();
                session_manager.new_session(call, Box::new(DefaultProvider::new(context)));
                Ok(())
            }
            "exit" => {
                let mut session_manager = self.session_manager_mutex.lock();
                session_manager.terminate(notification.session_id);
                Ok(())
            }
            _ => Err(anyhow::anyhow!("Unknown notification: {notification:?}")),
        }
    }

    /// Process the method call message from Vim.
    async fn process_method_call(&self, method_call: MethodCall) -> Result<Option<Value>> {
        use super::ProviderEvent::*;

        let msg = method_call;

        if msg.method != "init_ext_map" {
            tracing::debug!(?msg, "==> stdio message(in)");
        }

        let value = match msg.method.as_str() {
            "init_ext_map" => Some(msg.parse_filetypedetect()),
            "preview/file" => Some(msg.preview_file().await?),
            "quickfix" => Some(msg.preview_quickfix().await?),

            "dumb_jump/on_init" => {
                let mut session_manager = self.session_manager_mutex.lock();
                let call = Call::MethodCall(msg);
                let context: SessionContext = call.clone().into();
                session_manager.new_session(call, Box::new(DumbJumpProvider::new(context)));
                None
            }
            "dumb_jump/on_typed" => {
                let mut session_manager = self.session_manager_mutex.lock();
                session_manager.send(msg.session_id, OnTyped(msg));
                None
            }
            "dumb_jump/on_move" => {
                let mut session_manager = self.session_manager_mutex.lock();
                session_manager.send(msg.session_id, OnMove(msg));
                None
            }

            "on_typed" => {
                let mut session_manager = self.session_manager_mutex.lock();
                session_manager.send(msg.session_id, OnTyped(msg));
                None
            }
            "on_move" => {
                let mut session_manager = self.session_manager_mutex.lock();
                session_manager.send(msg.session_id, OnMove(msg));
                None
            }

            "recent_files/on_init" => {
                let mut session_manager = self.session_manager_mutex.lock();
                let call = Call::MethodCall(msg);
                let context: SessionContext = call.clone().into();
                session_manager.new_session(call, Box::new(RecentFilesProvider::new(context)));
                None
            }
            "recent_files/on_typed" => {
                let mut session_manager = self.session_manager_mutex.lock();
                session_manager.send(msg.session_id, OnTyped(msg));
                None
            }
            "recent_files/on_move" => {
                let mut session_manager = self.session_manager_mutex.lock();
                session_manager.send(msg.session_id, OnMove(msg));
                None
            }

            "filer/on_init" => {
                let mut session_manager = self.session_manager_mutex.lock();
                let call = Call::MethodCall(msg);
                let context: SessionContext = call.clone().into();
                session_manager.new_session(call, Box::new(FilerProvider::new(context)));
                None
            }
            "filer/on_move" => {
                let mut session_manager = self.session_manager_mutex.lock();
                session_manager.send(msg.session_id, OnMove(msg));
                None
            }
            "filer/on_typed" => {
                let mut session_manager = self.session_manager_mutex.lock();
                // TODO: send_and_wait_result
                session_manager.send(msg.session_id, OnMove(msg));
                None
            }

            _ => Some(json!({
                "error": format!("Unknown method call: {}", msg.method)
            })),
        };

        Ok(value)
    }
}
