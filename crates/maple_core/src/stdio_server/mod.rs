mod diagnostics_worker;
mod input;
mod job;
mod plugin;
mod provider;
mod request_handler;
mod service;
mod vim;
mod winbar;

pub use self::input::InputHistory;
use self::input::{ActionEvent, Event, ProviderEvent};
use self::plugin::PluginId;
pub use self::provider::SearchProgressor;
use self::provider::{create_provider, Context, ProviderError};
use self::service::ServiceManager;
pub use self::vim::Vim;
use self::vim::{initialize_filetype_map, VimError, VimResult};
use parking_lot::Mutex;
use rpc::vim::VimMessage;
use rpc::{RpcNotification, RpcRequest};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::{BufReader, BufWriter};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::time::Instant;
use types::PLUGIN_ACTION_SEPARATOR;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("`session_id` not found in params")]
    MissingSessionId,
    #[error("failed to parse: {0}")]
    Parse(String),
    #[error("failed to parse action from `{0:?}`")]
    ParseAction(RpcNotification),
    #[error("{0}")]
    Other(String),
    #[error(transparent)]
    Vim(#[from] VimError),
    #[error(transparent)]
    Provider(#[from] ProviderError),
    #[error(transparent)]
    Rpc(#[from] rpc::Error),
    #[error(transparent)]
    IO(#[from] std::io::Error),
    #[error(transparent)]
    ParseInt(#[from] std::num::ParseIntError),
}

// Do the initialization on the Vim end on startup.
async fn initialize_client(vim: Vim, actions: Vec<&str>, config_err: ConfigError) -> VimResult<()> {
    config_err.notify_error(&vim)?;

    let (mut other_actions, mut system_actions): (Vec<_>, Vec<_>) = actions
        .into_iter()
        .partition(|action| action.contains(PLUGIN_ACTION_SEPARATOR));
    other_actions.sort();
    system_actions.sort();
    let mut clap_actions = system_actions;
    clap_actions.extend(other_actions);
    vim.set_var("g:clap_actions", json![clap_actions])?;

    // The output of `autocmd filetypedetect` could be incomplete as the
    // filetype won't be instantly initialized, thus the current workaround
    // is to introduce some delay.
    //
    // TODO: parse filetype.vim
    tokio::time::sleep(Duration::from_millis(1000)).await;

    let output: String = vim
        .call("execute", json!(["autocmd filetypedetect"]))
        .await?;
    let ext_map = initialize_filetype_map(&output);
    vim.exec("clap#ext#set", json![ext_map])?;

    tracing::debug!("Client initialized successfully");

    Ok(())
}

struct InitializedService {
    callable_actions: Vec<&'static str>,
    plugin_actions: HashMap<PluginId, Vec<String>>,
    service_manager: ServiceManager,
}

/// Create a new service, with plugins registered from the config file.
fn initialize_service(vim: Vim) -> InitializedService {
    use self::diagnostics_worker::initialize_diagnostics_worker;
    use self::plugin::{
        ActionType, ClapPlugin, ColorizerPlugin, CtagsPlugin, DiagnosticsPlugin, GitPlugin,
        LinterPlugin, LspPlugin, MarkdownPlugin, SyntaxPlugin, SystemPlugin, WordHighlighterPlugin,
    };

    let mut callable_actions = Vec::new();
    let mut plugin_actions = HashMap::new();

    let mut service_manager = ServiceManager::default();

    let mut register_plugin = |plugin: Box<dyn ClapPlugin>, debounce: Option<Duration>| {
        callable_actions.extend(
            plugin
                .actions(ActionType::Callable)
                .iter()
                .map(|a| a.method),
        );

        let (plugin_id, actions) = service_manager.register_plugin(plugin, debounce);
        plugin_actions.insert(plugin_id, actions);
    };

    register_plugin(Box::new(SystemPlugin::new(vim.clone())), None);
    register_plugin(Box::new(SyntaxPlugin::new(vim.clone())), None);

    let plugin_config = &maple_config::config().plugin;

    if plugin_config.lsp.enable || plugin_config.linter.enable {
        let diagnostics_worker_msg_sender = initialize_diagnostics_worker(vim.clone());

        register_plugin(
            Box::new(DiagnosticsPlugin::new(
                vim.clone(),
                diagnostics_worker_msg_sender.clone(),
            )),
            None,
        );

        if plugin_config.lsp.enable {
            register_plugin(
                Box::new(LspPlugin::new(
                    vim.clone(),
                    diagnostics_worker_msg_sender.clone(),
                )),
                None,
            );
        }

        if plugin_config.linter.enable {
            register_plugin(
                Box::new(LinterPlugin::new(
                    vim.clone(),
                    diagnostics_worker_msg_sender,
                )),
                Some(Duration::from_millis(100)),
            );
        }
    }

    if plugin_config.colorizer.enable {
        register_plugin(
            Box::new(ColorizerPlugin::new(vim.clone())),
            Some(Duration::from_millis(100)),
        );
    }

    if plugin_config.word_highlighter.enable {
        register_plugin(Box::new(WordHighlighterPlugin::new(vim.clone())), None);
    }

    if plugin_config.git.enable {
        register_plugin(Box::new(GitPlugin::new(vim.clone())), None);
    }

    if plugin_config.ctags.enable {
        if crate::tools::ctags::CTAGS_BIN.is_available() {
            register_plugin(Box::new(CtagsPlugin::new(vim.clone())), None);
        } else {
            tracing::warn!("Failed to register ctags plugin as ctags executable not found");
        }
    }

    if plugin_config.markdown.enable {
        register_plugin(Box::new(MarkdownPlugin::new(vim)), None);
    }

    InitializedService {
        callable_actions,
        plugin_actions,
        service_manager,
    }
}

pub struct ConfigError {
    pub maybe_toml_err: Option<toml::de::Error>,
    pub maybe_log_target_err: Option<String>,
}

impl ConfigError {
    fn notify_error(self, vim: &Vim) -> VimResult<()> {
        if let Some(err) = self.maybe_toml_err {
            vim.echo_warn(format!(
                "Using default Config due to the error in {}: {err}",
                maple_config::config_file().display()
            ))?;
        }

        if let Some(err) = self.maybe_log_target_err {
            vim.echo_warn(err)?;
        }

        Ok(())
    }
}

/// Starts and keep running the server on top of stdio.
pub async fn start(config_err: ConfigError) {
    // TODO: setup test framework using vim_message_sender.
    let (vim_message_sender, vim_message_receiver) = tokio::sync::mpsc::unbounded_channel();

    let rpc_client = Arc::new(rpc::vim::RpcClient::new(
        BufReader::new(std::io::stdin()),
        BufWriter::new(std::io::stdout()),
        vim_message_sender.clone(),
    ));

    let vim = Vim::new(rpc_client);

    Backend::new(vim, config_err)
        .run(vim_message_receiver)
        .await;
}

#[derive(Clone)]
struct Backend {
    vim: Vim,
    plugin_actions: Arc<Mutex<HashMap<PluginId, Vec<String>>>>,
    service_manager: Arc<Mutex<ServiceManager>>,
}

impl Backend {
    /// Creates a new instance of [`Backend`].
    fn new(vim: Vim, config_err: ConfigError) -> Self {
        let InitializedService {
            callable_actions,
            plugin_actions,
            service_manager,
        } = initialize_service(vim.clone());

        tokio::spawn({
            let vim = vim.clone();
            async move {
                if let Err(e) = initialize_client(vim, callable_actions, config_err).await {
                    tracing::error!(error = ?e, "Failed to initialize Client")
                }
            }
        });

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
    async fn do_process_notification(&self, notification: RpcNotification) -> Result<(), Error> {
        let maybe_session_id = notification.session_id();

        let parse_action = |notification: RpcNotification| -> Result<ActionEvent, Error> {
            for (plugin_id, actions) in self.plugin_actions.lock().iter() {
                if actions.contains(&notification.method) {
                    return Ok((*plugin_id, notification.into()));
                }
            }
            Err(Error::ParseAction(notification))
        };

        match Event::parse_notification(notification, parse_action)? {
            Event::NewProvider(params) => {
                let session_id = maybe_session_id.ok_or(Error::MissingSessionId)?;
                let ctx = Context::new(params, self.vim.clone()).await?;
                let provider = create_provider(&ctx).await?;
                self.service_manager
                    .lock()
                    .new_provider(session_id, provider, ctx);
            }
            Event::ProviderWorker(provider_event) => match provider_event {
                ProviderEvent::Exit => {
                    let session_id = maybe_session_id.ok_or(Error::MissingSessionId)?;
                    self.service_manager.lock().notify_provider_exit(session_id);
                }
                to_send => {
                    let session_id = maybe_session_id.ok_or(Error::MissingSessionId)?;
                    self.service_manager
                        .lock()
                        .notify_provider(session_id, to_send);
                }
            },
            Event::Key(key_event) => {
                let session_id = maybe_session_id.ok_or(Error::MissingSessionId)?;
                self.service_manager
                    .lock()
                    .notify_provider(session_id, ProviderEvent::Key(key_event));
            }
            Event::Autocmd(autocmd_event) => {
                self.service_manager.lock().notify_plugins(autocmd_event);
            }
            Event::Action((plugin_id, plugin_action)) => {
                if plugin::SystemPlugin::is_list_plugins(plugin_id, &plugin_action) {
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
            let id = rpc_request.id.clone();

            match client.do_process_request(rpc_request).await {
                Ok(Some(result)) => {
                    // Send back the result of method call.
                    if let Err(err) = client.vim.send_response(id.clone(), Ok(result)) {
                        tracing::debug!(%id, ?err, "Failed to send the output result");
                    }
                }
                Ok(None) => {}
                Err(err) => {
                    tracing::error!(%id, ?err, "Error at processing Vim RpcRequest");
                }
            }
        });
    }

    async fn do_process_request(&self, rpc_request: RpcRequest) -> Result<Option<Value>, Error> {
        let msg = rpc_request;

        let value = match msg.method.as_str() {
            "preview/file" => Some(request_handler::preview_file(msg).await?),
            "quickfix" => Some(request_handler::preview_quickfix(msg).await?),
            _ => Some(json!({
                "error": format!("Unknown request: {}", msg.method)
            })),
        };

        Ok(value)
    }
}
