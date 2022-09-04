mod impls;
mod job;
mod provider;
mod rpc;
mod session;
mod state;
mod types;
mod vim;

use std::io::{BufReader, BufWriter};
use std::ops::Deref;
use std::sync::Arc;

use anyhow::{anyhow, Result};
use once_cell::sync::OnceCell;
use parking_lot::Mutex;
use serde_json::{json, Value};
use tokio::sync::mpsc::UnboundedReceiver;

use self::impls::dumb_jump::DumbJumpProvider;
use self::impls::filer::FilerProvider;
use self::impls::recent_files::RecentFilesProvider;
use self::impls::DefaultProvider;
use self::provider::{ClapProvider, ProviderEvent};
use self::rpc::{Call, MethodCall, Notification, RpcClient};
use self::session::{SessionContext, SessionManager};
use self::state::State;
use self::types::GlobalEnv;
use self::vim::Vim;

static GLOBAL_ENV: OnceCell<GlobalEnv> = OnceCell::new();

/// Ensure GLOBAL_ENV has been instalized before using it.
pub fn global() -> impl Deref<Target = GlobalEnv> {
    if let Some(x) = GLOBAL_ENV.get() {
        x
    } else if cfg!(debug_assertions) {
        panic!("Uninitalized static: GLOBAL_ENV")
    } else {
        unreachable!("Never forget to intialize before using it!")
    }
}

/// Starts and keep running the server on top of stdio.
pub async fn start() {
    let (call_tx, call_rx) = tokio::sync::mpsc::unbounded_channel();

    let rpc_client = Arc::new(RpcClient::new(
        BufReader::new(std::io::stdin()),
        BufWriter::new(std::io::stdout()),
        call_tx.clone(),
    ));

    let state = State::new(call_tx, rpc_client.clone());
    let session_client = Client::new(state, rpc_client);
    session_client.loop_call(call_rx).await;
}

#[derive(Clone)]
struct Client {
    vim: Vim,
    state_mutex: Arc<Mutex<State>>,
    session_manager_mutex: Arc<Mutex<SessionManager>>,
}

impl Client {
    /// Creates a new instnace of [`Client`].
    fn new(state: State, rpc_client: Arc<RpcClient>) -> Self {
        let vim = Vim::new(rpc_client);
        Self {
            vim,
            state_mutex: Arc::new(Mutex::new(state)),
            session_manager_mutex: Arc::new(Mutex::new(SessionManager::default())),
        }
    }

    /// Entry of the bridge between Vim and Rust.
    ///
    /// Handle the message actively initiated from Vim.
    async fn loop_call(self, mut rx: UnboundedReceiver<Call>) {
        while let Some(call) = rx.recv().await {
            let session_client = self.clone();
            tokio::spawn(async move {
                match call {
                    Call::Notification(notification) => {
                        if let Err(e) = session_client.process_notification(notification).await {
                            tracing::error!(error = ?e, "Error at handling a Vim Notification");
                        }
                    }
                    Call::MethodCall(method_call) => {
                        let id = method_call.id;

                        match session_client.process_method_call(method_call).await {
                            Ok(Some(result)) => {
                                // Send back the result of method call.
                                let state = session_client.state_mutex.lock();
                                if let Err(e) = state.vim.send(id, Ok(result)) {
                                    tracing::debug!(error = ?e, "Failed to send the output result");
                                }
                            }
                            Ok(None) => {}
                            Err(e) => {
                                tracing::error!(error = ?e, "Error at handling a Vim MethodCall");
                            }
                        }
                    }
                }
            });
        }
    }

    /// Process the notification message from Vim.
    async fn process_notification(&self, notification: Notification) -> Result<()> {
        use ProviderEvent::*;

        let session_id = || {
            notification
                .session_id
                .ok_or_else(|| anyhow!("Notification must contain `session_id`"))
        };

        match notification.method.as_str() {
            "initialize_global_env" => notification.initialize_global_env(self.vim.clone()).await, // should be called only once.
            "note_recent_files" => notification.note_recent_file().await,

            "on_init" => {
                let session_id = session_id()?;

                let vim = self.vim.clone();
                let context = SessionContext::new(notification.params, vim).await?;
                let provider: Box<dyn ClapProvider> = match self.vim.provider_id().await?.as_str() {
                    "filer" => Box::new(FilerProvider::new(context)),
                    "dumb_jump" => Box::new(DumbJumpProvider::new(context)),
                    "recent_files" => Box::new(RecentFilesProvider::new(context)),
                    _ => Box::new(DefaultProvider::new(context)),
                };

                let session_manager = self.session_manager_mutex.clone();
                let mut session_manager = session_manager.lock();
                session_manager.new_session(session_id, provider);

                Ok(())
            }

            "on_typed" => {
                let session_manager = self.session_manager_mutex.lock();
                session_manager.send(session_id()?, OnTyped);
                Ok(())
            }

            "on_move" => {
                let session_manager = self.session_manager_mutex.lock();
                session_manager.send(session_id()?, OnMove);
                Ok(())
            }

            "exit" => {
                let mut session_manager = self.session_manager_mutex.lock();
                session_manager.terminate(session_id()?);
                Ok(())
            }
            _ => Err(anyhow!("Unknown notification: {notification:?}")),
        }
    }

    /// Process the method call message from Vim.
    async fn process_method_call(&self, method_call: MethodCall) -> Result<Option<Value>> {
        let msg = method_call;

        let value = match msg.method.as_str() {
            "preview/file" => Some(msg.preview_file().await?),
            "quickfix" => Some(msg.preview_quickfix().await?),

            // Deprecated but not remove them for now.
            "on_move" => {
                let session_manager = self.session_manager_mutex.lock();
                session_manager.send(msg.session_id, ProviderEvent::OnMove);
                None
            }

            _ => Some(json!({
                "error": format!("Unknown method call: {}", msg.method)
            })),
        };

        Ok(value)
    }
}
