mod handler;
mod input;
mod job;
mod plugin;
mod provider;
mod service;
mod vim;

pub use self::input::InputHistory;
use self::input::{ActionEvent, Event, ProviderEvent};
use self::plugin::{
    ActionType, ClapPlugin, CtagsPlugin, CursorWordHighlighter, GitPlugin, LinterPlugin,
    MarkdownPlugin, PluginId, SystemPlugin,
};
use self::provider::{create_provider, Context};
use self::service::ServiceManager;
use self::vim::initialize_syntax_map;
pub use self::vim::{Vim, VimProgressor};
use anyhow::{anyhow, Result};
use parking_lot::Mutex;
use rpc::{RpcClient, RpcNotification, RpcRequest, VimMessage};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::{BufReader, BufWriter};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::time::Instant;

// Do the initialization on startup.
async fn initialize(
    vim: Vim,
    actions: Vec<&str>,
    config_err: Option<toml::de::Error>,
) -> Result<()> {
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

    vim.set_var("g:clap_actions", json![actions])?;

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

    let mut callable_action_methods = Vec::new();
    let mut all_actions = HashMap::new();

    let mut service_manager = ServiceManager::default();

    let mut register_plugin = |plugin: Box<dyn ClapPlugin>, debounce: Option<Duration>| {
        callable_action_methods.extend(
            plugin
                .actions(ActionType::Callable)
                .iter()
                .map(|a| a.method),
        );

        let (plugin_id, actions) = service_manager.register_plugin(plugin, debounce);
        all_actions.insert(plugin_id, actions);
    };

    register_plugin(Box::new(SystemPlugin::new(vim.clone())), None);
    register_plugin(Box::new(GitPlugin::new(vim.clone())), None);
    register_plugin(
        Box::new(LinterPlugin::new(vim.clone())),
        Some(Duration::from_millis(100)),
    );

    let plugin_config = &crate::config::config().plugin;

    if plugin_config.ctags.enable {
        register_plugin(Box::new(CtagsPlugin::new(vim.clone())), None);
    }

    if plugin_config.markdown.enable {
        register_plugin(Box::new(MarkdownPlugin::new(vim.clone())), None);
    }

    if plugin_config.cursor_word_highlighter.enable {
        register_plugin(Box::new(CursorWordHighlighter::new(vim.clone())), None);
    }

    tokio::spawn({
        let vim = vim.clone();
        async move {
            if let Err(e) = initialize(vim, callable_action_methods, config_err).await {
                tracing::error!(error = ?e, "Failed to initialize Client")
            }
        }
    });

    Client::new(vim, service_manager, all_actions)
        .run(vim_message_receiver)
        .await;
}

#[derive(Clone)]
struct Client {
    vim: Vim,
    plugin_actions: Arc<Mutex<HashMap<PluginId, Vec<String>>>>,
    service_manager: Arc<Mutex<ServiceManager>>,
}

impl Client {
    /// Creates a new instnace of [`Client`].
    fn new(
        vim: Vim,
        service_manager: ServiceManager,
        plugin_actions: HashMap<PluginId, Vec<String>>,
    ) -> Self {
        Self {
            vim,
            plugin_actions: Arc::new(Mutex::new(plugin_actions)),
            service_manager: Arc::new(Mutex::new(service_manager)),
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
                                    if notification.method == "new_provider" {
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
                        self.service_manager.lock().try_exit(last_session_id);
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
            if self.service_manager.lock().exists(session_id) {
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

        let action_parser = |notification: RpcNotification| -> Result<ActionEvent> {
            for (plugin_id, actions) in self.plugin_actions.lock().iter() {
                if actions.contains(&notification.method) {
                    return Ok((*plugin_id, notification.into()));
                }
            }
            Err(anyhow!("Failed to parse {notification:?}"))
        };

        match Event::parse_notification(notification, action_parser)? {
            Event::NewProvider(params) => {
                let session_id =
                    maybe_session_id.ok_or_else(|| anyhow!("`session_id` not found in Params"))?;
                let ctx = Context::new(params, self.vim.clone()).await?;
                let provider = create_provider(&ctx).await?;
                self.service_manager
                    .lock()
                    .new_provider(session_id, provider, ctx);
            }
            Event::ProviderWorker(provider_event) => match provider_event {
                ProviderEvent::Exit => {
                    let session_id = maybe_session_id
                        .ok_or_else(|| anyhow!("`session_id` not found in Params"))?;
                    self.service_manager.lock().notify_provider_exit(session_id);
                }
                to_send => {
                    let session_id = maybe_session_id
                        .ok_or_else(|| anyhow!("`session_id` not found in Params"))?;
                    self.service_manager
                        .lock()
                        .notify_provider(session_id, to_send);
                }
            },
            Event::Key(key_event) => {
                let session_id =
                    maybe_session_id.ok_or_else(|| anyhow!("`session_id` not found in Params"))?;
                self.service_manager
                    .lock()
                    .notify_provider(session_id, ProviderEvent::Key(key_event));
            }
            Event::Autocmd(autocmd_event) => {
                self.service_manager.lock().notify_plugins(autocmd_event);
            }
            Event::Action((plugin_id, plugin_action)) => {
                if plugin_id == PluginId::System && plugin_action.method == "list-plugins" {
                    let lines = self
                        .service_manager
                        .lock()
                        .plugins
                        .keys()
                        .map(|p| p.to_string())
                        .collect::<Vec<_>>();
                    self.vim.echo_info(lines.join(","))?;
                    return Ok(());
                }
                self.service_manager
                    .lock()
                    .notify_plugin_action(plugin_id, plugin_action);
            }
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
