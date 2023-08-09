mod handler;
mod input;
mod job;
mod plugin;
mod provider;
mod service;
mod vim;

pub use self::input::InputHistory;
use self::input::{Event, PluginEvent, ProviderEvent};
use self::plugin::{ClapPlugin, CtagsPlugin, CursorWordHighlighter};
use self::provider::{create_provider, Context};
use self::service::ServiceManager;
use self::vim::initialize_syntax_map;
pub use self::vim::{Vim, VimProgressor};
use crate::stdio_server::input::Action;
use anyhow::{anyhow, Result};
use parking_lot::Mutex;
use rpc::{RpcClient, RpcNotification, RpcRequest, VimMessage};
use serde_json::{json, Value};
use std::io::{BufReader, BufWriter};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::time::Instant;

// Do the initialization on startup.
async fn initialize(vim: Vim, config_err: Option<toml::de::Error>) -> Result<()> {
    if let Some(err) = config_err {
        vim.echo_warn(format!(
            "Using default Config due to the error in {}: {err}",
            crate::config::config_file().display()
        ))?;
    }

    // The output of `autocmd filetypedetect` could be incomplete as the
    // filetype won't be instantly initialized, thus the current workaround
    // is to introduce some delay.
    //
    // TODO: parse filetype.vim
    tokio::time::sleep(Duration::from_millis(1000)).await;

    let output: String = vim
        .call("execute", json!(["autocmd filetypedetect"]))
        .await?;
    let ext_map = initialize_syntax_map(&output);
    vim.exec("clap#ext#set", json![ext_map])?;

    const ACTIONS: &[&str] = &["open-config", "generate-toc", "update-toc", "delete-toc"];
    vim.set_var("g:clap_actions", json![ACTIONS])?;

    tracing::debug!("Client initialized successfully");

    Ok(())
}

/// Starts and keep running the server on top of stdio.
pub async fn start(config_err: Option<toml::de::Error>) {
    // TODO: setup test framework using vim_message_sender.
    let (vim_message_sender, vim_message_receiver) = tokio::sync::mpsc::unbounded_channel();

    let rpc_client = Arc::new(RpcClient::new(
        BufReader::new(std::io::stdin()),
        BufWriter::new(std::io::stdout()),
        vim_message_sender.clone(),
    ));

    let vim = Vim::new(rpc_client);

    tokio::spawn({
        let vim = vim.clone();
        async move {
            if let Err(e) = initialize(vim, config_err).await {
                tracing::error!(error = ?e, "Failed to initialize Client")
            }
        }
    });

    Client::new(vim).run(vim_message_receiver).await;
}

#[derive(Clone)]
struct Client {
    vim: Vim,
    service_manager_mutex: Arc<Mutex<ServiceManager>>,
}

impl Client {
    /// Creates a new instnace of [`Client`].
    fn new(vim: Vim) -> Self {
        let mut service_manager = ServiceManager::default();
        service_manager.new_plugin(Box::new(CtagsPlugin::new(vim.clone())) as Box<dyn ClapPlugin>);
        if crate::config::config().plugin.highlight_cursor_word.enable {
            service_manager.new_plugin(
                Box::new(CursorWordHighlighter::new(vim.clone())) as Box<dyn ClapPlugin>
            );
        }
        Self {
            vim,
            service_manager_mutex: Arc::new(Mutex::new(service_manager)),
        }
    }

    /// Entry of the bridge between Vim and Rust.
    ///
    /// Handle the messages actively initiated from Vim.
    async fn run(self, mut rx: UnboundedReceiver<VimMessage>) {
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
                                VimMessage::Request(rpc_request) => self.process_request(rpc_request),
                                VimMessage::Notification(notification) => {
                                    // Avoid spawn too frequently if user opens and
                                    // closes the provider frequently in a very short time.
                                    if notification.method == "new_session" {
                                        pending_notification.replace(notification);

                                        notification_dirty = true;
                                        notification_timer
                                            .as_mut()
                                            .reset(Instant::now() + notification_delay);
                                    } else {
                                        self.process_notification(notification);
                                    }
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
                            .session_id()
                            .unwrap_or_default()
                            .saturating_sub(1);
                        self.service_manager_mutex.lock().try_exit(last_session_id);
                        let session_id = notification.session_id();
                        if let Err(err) = self.do_process_notification(notification).await {
                            tracing::error!(?session_id, ?err, "Error at processing Vim Notification");
                        }
                    }
                }
            }
        }
    }

    fn process_notification(&self, notification: RpcNotification) {
        if let Some(session_id) = notification.session_id() {
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
    async fn do_process_notification(&self, notification: RpcNotification) -> Result<()> {
        let maybe_session_id = notification.session_id();
        match Event::parse_notification(notification) {
            Event::Provider(provider_event) => match provider_event {
                ProviderEvent::NewSession(params) => {
                    let provider_id = self.vim.provider_id().await?;
                    let session_id = maybe_session_id
                        .ok_or_else(|| anyhow!("`session_id` not found in Params"))?;
                    let ctx = Context::new(params, self.vim.clone()).await?;
                    let provider = create_provider(&provider_id, &ctx).await?;
                    self.service_manager_mutex
                        .lock()
                        .new_provider(session_id, provider, ctx);
                }
                ProviderEvent::Exit => {
                    let session_id = maybe_session_id
                        .ok_or_else(|| anyhow!("`session_id` not found in Params"))?;
                    self.service_manager_mutex
                        .lock()
                        .notify_provider_exit(session_id);
                }
                to_send => {
                    let session_id = maybe_session_id
                        .ok_or_else(|| anyhow!("`session_id` not found in Params"))?;
                    self.service_manager_mutex
                        .lock()
                        .notify_provider(session_id, to_send);
                }
            },
            Event::Key(key_event) => {
                let session_id =
                    maybe_session_id.ok_or_else(|| anyhow!("`session_id` not found in Params"))?;
                self.service_manager_mutex
                    .lock()
                    .notify_provider(session_id, ProviderEvent::Key(key_event));
            }
            Event::Autocmd(autocmd_event) => {
                self.service_manager_mutex
                    .lock()
                    .notify_plugins(PluginEvent::Autocmd(autocmd_event));
            }
            Event::Action(action) => self.handle_action(action).await?,
        }

        Ok(())
    }

    async fn handle_action(&self, action: Action) -> Result<()> {
        match action.command.as_str() {
            "note_recent_files" => {
                let bufnr: Vec<usize> = action.params.parse()?;
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
                let shiftwidth = self
                    .vim
                    .call("getbufvar", json!(["", "&shiftwidth"]))
                    .await?;
                let mut toc = plugin::generate_toc(file, curlnum, shiftwidth)?;
                let prev_line = self.vim.curbufline(curlnum - 1).await?;
                if !prev_line.map(|line| line.is_empty()).unwrap_or(false) {
                    toc.push_front(Default::default());
                }
                self.vim
                    .exec("append_and_write", json!([curlnum - 1, toc]))?;
            }
            "update-toc" => {
                let file = self.vim.current_buffer_path().await?;
                let bufnr = self.vim.current_bufnr().await?;
                if let Some((start, end)) = plugin::find_toc_range(&file)? {
                    let shiftwidth = self
                        .vim
                        .call("getbufvar", json!(["", "&shiftwidth"]))
                        .await?;
                    // TODO: skip update if the new doc is the same as the old one.
                    let new_toc = plugin::generate_toc(file, start + 1, shiftwidth)?;
                    self.vim
                        .exec("deletebufline", json!([bufnr, start + 1, end + 1]))?;
                    self.vim.exec("append_and_write", json!([start, new_toc]))?;
                }
            }
            "delete-toc" => {
                let file = self.vim.current_buffer_path().await?;
                let bufnr = self.vim.current_bufnr().await?;
                if let Some((start, end)) = plugin::find_toc_range(file)? {
                    self.vim
                        .exec("deletebufline", json!([bufnr, start + 1, end + 1]))?;
                }
            }
            _ => return Err(anyhow!("Unknown action: {action:?}")),
        }

        Ok(())
    }

    /// Process [`RpcRequest`] initiated from Vim.
    fn process_request(&self, rpc_request: RpcRequest) {
        let client = self.clone();

        tokio::spawn(async move {
            let id = rpc_request.id;

            match client.do_process_request(rpc_request).await {
                Ok(Some(result)) => {
                    // Send back the result of method call.
                    if let Err(err) = client.vim.send_response(id, Ok(result)) {
                        tracing::debug!(id, ?err, "Failed to send the output result");
                    }
                }
                Ok(None) => {}
                Err(err) => {
                    tracing::error!(id, ?err, "Error at processing Vim RpcRequest");
                }
            }
        });
    }

    async fn do_process_request(&self, rpc_request: RpcRequest) -> Result<Option<Value>> {
        let msg = rpc_request;

        let value = match msg.method.as_str() {
            "preview/file" => Some(handler::messages::preview_file(msg).await?),
            "quickfix" => Some(handler::messages::preview_quickfix(msg).await?),
            _ => Some(json!({
                "error": format!("Unknown request: {}", msg.method)
            })),
        };

        Ok(value)
    }
}
