mod handler;
mod input;
mod job;
mod provider;
mod rpc;
mod session;
mod state;
mod vim;

pub use self::input::InputHistory;
use self::input::{Event, ProviderEvent};
use self::provider::{create_provider, Context};
use self::rpc::{Call, MethodCall, Notification, RpcClient};
use self::session::SessionManager;
use self::state::State;
pub use self::vim::{Vim, VimProgressor};
use anyhow::{anyhow, Result};
use parking_lot::Mutex;
use serde_json::{json, Value};
use std::io::{BufReader, BufWriter};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::time::Instant;

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
        // If the debounce timer isn't active, it will be set to expire "never",
        // which is actually just 1 year in the future.
        const NEVER: Duration = Duration::from_secs(365 * 24 * 60 * 60);

        let mut pending_notification = None;
        let mut notification_dirty = false;
        let notification_delay = Duration::from_millis(200);
        let notification_timer = tokio::time::sleep(NEVER);
        tokio::pin!(notification_timer);

        loop {
            tokio::select! {
                maybe_call = rx.recv() => {
                    match maybe_call {
                        Some(call) => {
                            match call {
                                Call::Notification(notification) => {
                                    // Avoid spawn too frequently if user opens and
                                    // closes the provider frequently in a very short time.
                                    match Event::from_method(&notification.method) {
                                        Event::Provider(ProviderEvent::NewSession) => {
                                            pending_notification.replace(notification);

                                            notification_dirty = true;
                                            notification_timer
                                                .as_mut()
                                                .reset(Instant::now() + notification_delay);
                                        }
                                        _ => {
                                            if let Some(session_id) = notification.session_id {
                                                if self.session_manager_mutex.lock().exists(session_id) {
                                                    let client = self.clone();

                                                    tokio::spawn(async move {
                                                        if let Err(err) =
                                                            client.process_notification(notification).await
                                                        {
                                                            tracing::error!(
                                                                ?session_id,
                                                                ?err,
                                                                "Error at processing Vim Notification"
                                                            );
                                                        }
                                                    });
                                                }
                                            }
                                        }
                                    }
                                }
                                Call::MethodCall(method_call) => {
                                    let client = self.clone();

                                    tokio::spawn(async move {
                                        let id = method_call.id;

                                        match client.process_method_call(method_call).await {
                                            Ok(Some(result)) => {
                                                // Send back the result of method call.
                                                let state = client.state_mutex.lock();
                                                if let Err(err) = state.vim.send(id, Ok(result)) {
                                                    tracing::debug!(?err, "Failed to send the output result");
                                                }
                                            }
                                            Ok(None) => {}
                                            Err(err) => {
                                                tracing::error!(?err, "Error at processing Vim MethodCall");
                                            }
                                        }
                                    });
                                }
                            }
                        }
                        None => break, // channel has closed.
                    }
                }
                _ = notification_timer.as_mut(), if notification_dirty => {
                    notification_dirty = false;
                    notification_timer.as_mut().reset(Instant::now() + NEVER);

                    if let Some(notification) = pending_notification.take() {
                        let last_session_id = notification
                            .session_id
                            .unwrap_or_default()
                            .saturating_sub(1);
                        self.session_manager_mutex.lock().try_exit(last_session_id);
                        let session_id = notification.session_id;
                        if let Err(err) = self.process_notification(notification).await {
                            tracing::error!(?session_id, ?err, "Error at processing Vim Notification");
                        }
                    }
                }
            }
        }
    }

    /// Process a Vim notification message.
    async fn process_notification(&self, notification: Notification) -> Result<()> {
        let session_id = || {
            notification
                .session_id
                .ok_or_else(|| anyhow!("Notification must contain `session_id` field"))
        };

        match Event::from_method(&notification.method) {
            Event::Provider(provider_event) => match provider_event {
                ProviderEvent::NewSession => {
                    let provider_id = self.vim.provider_id().await?;
                    let ctx = Context::new(notification.params, self.vim.clone()).await?;
                    let provider = create_provider(&provider_id, &ctx).await?;
                    let session_manager = self.session_manager_mutex.clone();
                    let mut session_manager = session_manager.lock();
                    session_manager.new_session(session_id()?, provider, ctx);
                }
                ProviderEvent::Exit => {
                    let mut session_manager = self.session_manager_mutex.lock();
                    session_manager.exit_session(session_id()?);
                }
                to_send => {
                    let session_manager = self.session_manager_mutex.lock();
                    session_manager.send(session_id()?, to_send);
                }
            },
            Event::Key(key_event) => {
                let session_manager = self.session_manager_mutex.lock();
                session_manager.send(session_id()?, ProviderEvent::Key(key_event));
            }
            Event::Other(other_method) => {
                match other_method.as_str() {
                    "initialize_global_env" => {
                        // Should be called only once.
                        notification.initialize(self.vim.clone()).await?;
                    }
                    "note_recent_files" => notification.note_recent_file().await?,
                    _ => return Err(anyhow!("Unknown notification: {notification:?}")),
                }
            }
        }

        Ok(())
    }

    /// Process a Vim method call message.
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
