mod handler;
mod input;
mod job;
mod plugin;
mod provider;
mod service;
mod state;
mod vim;

pub use self::input::InputHistory;
use self::input::{Event, PluginEvent, ProviderEvent};
use self::plugin::{ClapPlugin, CursorWordHighlighter};
use self::provider::{create_provider, Context};
use self::service::ServiceManager;
use self::state::State;
use self::vim::initialize_syntax_map;
pub use self::vim::{Vim, VimProgressor};
use anyhow::{anyhow, Result};
use parking_lot::Mutex;
use rpc::{Call, MethodCall, Notification, RpcClient};
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
    service_manager_mutex: Arc<Mutex<ServiceManager>>,
}

impl Client {
    /// Creates a new instnace of [`Client`].
    fn new(state: State, rpc_client: Arc<RpcClient>) -> Self {
        let vim = Vim::new(rpc_client);
        let mut service_manager = ServiceManager::default();
        if crate::config::config().plugin.highlight_cursor_word.enable {
            service_manager.new_plugin(
                Box::new(CursorWordHighlighter::new(vim.clone())) as Box<dyn ClapPlugin>
            );
        }
        Self {
            vim,
            state_mutex: Arc::new(Mutex::new(state)),
            service_manager_mutex: Arc::new(Mutex::new(service_manager)),
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
        let notification_delay = Duration::from_millis(50);
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
                                        _ => self.process_notification(notification),
                                    }
                                }
                                Call::MethodCall(method_call) => self.process_method_call(method_call),
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
                        self.service_manager_mutex.lock().try_exit(last_session_id);
                        let session_id = notification.session_id;
                        if let Err(err) = self.do_process_notification(notification).await {
                            tracing::error!(?session_id, ?err, "Error at processing Vim Notification");
                        }
                    }
                }
            }
        }
    }

    fn process_notification(&self, notification: Notification) {
        if let Some(session_id) = notification.session_id {
            if self.service_manager_mutex.lock().exists(session_id) {
                let client = self.clone();

                tokio::spawn(async move {
                    if let Err(err) = client.do_process_notification(notification).await {
                        tracing::error!(?session_id, ?err, "Error at processing Vim Notification");
                    }
                });
            }
        } else {
            let client = self.clone();
            tokio::spawn(async move {
                if let Err(err) = client.do_process_notification(notification).await {
                    tracing::error!(?err, "Error at processing Vim Notification");
                }
            });
        }
    }

    /// Actually process a Vim notification message.
    async fn do_process_notification(&self, notification: Notification) -> Result<()> {
        let provider_session_id = || {
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
                    self.service_manager_mutex.lock().new_provider(
                        provider_session_id()?,
                        provider,
                        ctx,
                    );
                }
                ProviderEvent::Exit => {
                    self.service_manager_mutex
                        .lock()
                        .notify_provider_exit(provider_session_id()?);
                }
                to_send => {
                    self.service_manager_mutex
                        .lock()
                        .notify_provider(provider_session_id()?, to_send);
                }
            },
            Event::Key(key_event) => {
                self.service_manager_mutex
                    .lock()
                    .notify_provider(provider_session_id()?, ProviderEvent::Key(key_event));
            }
            Event::Autocmd(autocmd) => {
                self.service_manager_mutex
                    .lock()
                    .notify_plugins(PluginEvent::Autocmd(autocmd));
            }
            Event::Action(action) => {
                match action.as_str() {
                    "initialize_global_env" => {
                        // Should be called only once.
                        let output: String = self
                            .vim
                            .call("execute", json!(["autocmd filetypedetect"]))
                            .await?;
                        let ext_map = initialize_syntax_map(&output);
                        self.vim.exec("clap#ext#set", json![ext_map])?;
                        tracing::debug!("Client initialized successfully");
                    }
                    "note_recent_files" => {
                        let bufnr: Vec<usize> = notification.params.parse()?;
                        let bufnr = bufnr
                            .first()
                            .ok_or(anyhow!("bufnr not found in `note_recent_file`"))?;
                        let file_path: String = self.vim.expand(format!("#{bufnr}:p")).await?;
                        handler::messages::note_recent_file(file_path)?
                    }
                    "open-config" => {
                        let config_file = crate::config::config_file();
                        self.vim
                            .exec("execute", format!("edit {}", config_file.display()))?;
                    }
                    "generate-toc" => {
                        let curlnum = self.vim.line(".").await?;
                        let file = self.vim.current_buffer_path().await?;
                        let mut toc = plugin::generate_toc(file, curlnum)?;
                        let prev_line = self.vim.curbufline(curlnum - 1).await?;
                        if !prev_line.map(|line| line.is_empty()).unwrap_or(false) {
                            toc.push_front(Default::default());
                        }
                        self.vim
                            .exec("append", serde_json::json!([curlnum - 1, toc]))?;
                    }
                    "update-toc" => {
                        let file = self.vim.current_buffer_path().await?;
                        let bufnr = self.vim.current_bufnr().await?;
                        if let Some((start, end)) = plugin::find_toc_range(&file)? {
                            let new_toc = plugin::generate_toc(file, start + 1)?;
                            self.vim.exec(
                                "deletebufline",
                                serde_json::json!([bufnr, start + 1, end + 1]),
                            )?;
                            self.vim
                                .exec("append", serde_json::json!([start + 1, new_toc]))?;
                        }
                    }
                    "delete-toc" => {
                        let file = self.vim.current_buffer_path().await?;
                        let bufnr = self.vim.current_bufnr().await?;
                        if let Some((start, end)) = plugin::find_toc_range(file)? {
                            self.vim.exec(
                                "deletebufline",
                                serde_json::json!([bufnr, start + 1, end + 1]),
                            )?;
                        }
                    }
                    _ => return Err(anyhow!("Unknown notification: {notification:?}")),
                }
            }
        }

        Ok(())
    }

    fn process_method_call(&self, method_call: MethodCall) {
        let client = self.clone();

        tokio::spawn(async move {
            let id = method_call.id;

            match client.do_process_method_call(method_call).await {
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

    /// Process a Vim method call message.
    async fn do_process_method_call(&self, method_call: MethodCall) -> Result<Option<Value>> {
        let msg = method_call;

        let value = match msg.method.as_str() {
            "preview/file" => Some(handler::messages::preview_file(msg).await?),
            "quickfix" => Some(handler::messages::preview_quickfix(msg).await?),

            // Deprecated but not remove them for now.
            "on_move" => {
                if let Some(session_id) = msg.session_id {
                    self.service_manager_mutex
                        .lock()
                        .notify_provider(session_id, ProviderEvent::OnMove);
                }
                None
            }

            _ => Some(json!({
                "error": format!("Unknown method call: {}", msg.method)
            })),
        };

        Ok(value)
    }
}
