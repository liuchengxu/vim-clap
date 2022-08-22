use std::sync::Arc;

use anyhow::Result;
use parking_lot::Mutex;
use serde_json::{json, Value};
use tokio::sync::mpsc::UnboundedReceiver;

use super::rpc::RpcClient;
use super::session::SessionManager;
use super::vim::Vim;
use super::Notification;
use crate::stdio_server::impls::dumb_jump::DumbJumpProvider;
use crate::stdio_server::impls::filer::FilerProvider;
use crate::stdio_server::impls::recent_files::RecentFilesProvider;
use crate::stdio_server::impls::DefaultProvider;
use crate::stdio_server::rpc::{Call, MethodCall};
use crate::stdio_server::session::{ClapProvider, SessionContext};
use crate::stdio_server::state::State;

#[derive(Clone)]
pub struct SessionClient {
    vim: Vim,
    pub state_mutex: Arc<Mutex<State>>,
    pub session_manager_mutex: Arc<Mutex<SessionManager>>,
}

impl SessionClient {
    /// Creates a new instnace of [`SessionClient`].
    pub fn new(state: State, rpc_client: Arc<RpcClient>) -> Self {
        let vim = Vim::new(rpc_client);
        Self {
            vim,
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
        use crate::stdio_server::session::ProviderEvent::*;

        let msg = method_call;

        if msg.method != "init_ext_map" {
            tracing::debug!(?msg, "==> stdio message(in)");
        }

        let value = match msg.method.as_str() {
            "init_ext_map" => Some(msg.parse_filetypedetect()),
            "preview/file" => Some(msg.preview_file().await?),
            "quickfix" => Some(msg.preview_quickfix().await?),

            "dumb_jump/on_init" | "recent_files/on_init" | "filer/on_init" => {
                let call = Call::MethodCall(msg);
                let context: SessionContext = call.clone().into();

                let provider_id = self.vim.current_provider_id().await?;
                let provider: Box<dyn ClapProvider> = match provider_id.as_str() {
                    "dumb_jump" => Box::new(DumbJumpProvider::new(context, self.vim.clone())),
                    "recent_files" => Box::new(RecentFilesProvider::new(context)),
                    "filer" => Box::new(FilerProvider::new(context)),
                    _ => Box::new(DefaultProvider::new(context)),
                };

                let session_manager = self.session_manager_mutex.clone();
                let mut session_manager = session_manager.lock();
                session_manager.new_session(call, provider);

                None
            }

            "on_typed" | "filer/on_typed" | "dumb_jump/on_typed" | "recent_files/on_typed" => {
                let session_manager = self.session_manager_mutex.lock();
                session_manager.send(msg.session_id, OnTyped(msg));
                None
            }

            "on_move" | "filer/on_move" | "dumb_jump/on_move" | "recent_files/on_move" => {
                let session_manager = self.session_manager_mutex.lock();
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
