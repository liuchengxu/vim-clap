mod handler;
mod job;
mod provider;
mod rpc;
mod session;
mod state;
mod types;
mod vim;

use self::provider::{create_provider, Event, ProviderContext, ProviderEvent};
use self::rpc::{Call, MethodCall, Notification, RpcClient};
use self::session::SessionManager;
use self::state::State;
pub use self::vim::Vim;
use anyhow::{anyhow, Result};
use parking_lot::Mutex;
use serde_json::{json, Value};
use std::io::{BufReader, BufWriter};
use std::sync::Arc;
use tokio::sync::mpsc::UnboundedReceiver;

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
                            tracing::error!(error = ?e, "Error at handling Vim Notification");
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
                                tracing::error!(error = ?e, "Error at handling Vim MethodCall");
                            }
                        }
                    }
                }
            });
        }
    }

    /// Process the notification message from Vim.
    async fn process_notification(&self, notification: Notification) -> Result<()> {
        let session_id = || {
            notification
                .session_id
                .ok_or_else(|| anyhow!("Notification must contain `session_id` field"))
        };

        match Event::from_method(&notification.method) {
            Event::Provider(provider_event) => match provider_event {
                ProviderEvent::Create => {
                    let vim = self.vim.clone();
                    let context = ProviderContext::new(notification.params, vim).await?;
                    let provider_id = self.vim.provider_id().await?;
                    let provider = create_provider(&provider_id, context);
                    let session_manager = self.session_manager_mutex.clone();
                    let mut session_manager = session_manager.lock();
                    session_manager.new_session(session_id()?, provider);
                }
                ProviderEvent::Terminate => {
                    let mut session_manager = self.session_manager_mutex.lock();
                    session_manager.terminate(session_id()?);
                }
                to_send => {
                    let session_manager = self.session_manager_mutex.lock();
                    session_manager.send(session_id()?, to_send);
                }
            },
            Event::Other(other_method) => {
                match other_method.as_str() {
                    "initialize_global_env" => {
                        // should be called only once.
                        notification.initialize(self.vim.clone()).await?;
                    }
                    "note_recent_files" => notification.note_recent_file().await?,
                    _ => return Err(anyhow!("Unknown notification: {notification:?}")),
                }
            }
        }

        Ok(())
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
