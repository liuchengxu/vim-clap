//! Each invocation of Clap provider is a session. When you exit the provider, the session ends.

use crate::stdio_server::input::{
    AutocmdEvent, AutocmdEventType, InternalProviderEvent, PluginAction, PluginEvent,
    ProviderEvent, ProviderEventSender,
};
use crate::stdio_server::plugin::{ActionType, ClapPlugin, PluginId};
use crate::stdio_server::provider::{ClapProvider, Context, ProviderId};
use rpc::Params;
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::fmt::Debug;
use std::ops::ControlFlow;
use std::time::Duration;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};
use tokio::time::Instant;

pub type ProviderSessionId = u64;

// Type alias here for readability.
type DebouncedProviderEvent = ProviderEvent;

#[derive(Debug)]
pub struct ProviderSession {
    ctx: Context,
    id: ProviderId,
    provider_session_id: ProviderSessionId,
    /// Each provider session can have its own message processing logic.
    provider: Box<dyn ClapProvider>,
    provider_events: UnboundedReceiver<DebouncedProviderEvent>,
}

struct DebounceTimer {
    last_emitted: Option<std::time::Instant>,
    debounce_period: Duration,
}

impl DebounceTimer {
    fn new(debounce_period: Duration) -> Self {
        Self {
            last_emitted: None,
            debounce_period,
        }
    }

    fn should_emit(&mut self) -> bool {
        let now = std::time::Instant::now();
        if self.last_emitted.is_none()
            || now.duration_since(self.last_emitted.expect("Must be Some as checked"))
                > self.debounce_period
        {
            self.last_emitted.replace(now);
            return true;
        }
        false
    }
}

impl ProviderSession {
    pub fn new(
        ctx: Context,
        provider_session_id: ProviderSessionId,
        provider: Box<dyn ClapProvider>,
    ) -> (Self, UnboundedSender<ProviderEvent>) {
        let (origin_provider_event_sender, mut origin_provider_event_receiver) =
            unbounded_channel();

        let (debounced_provider_event_sender, debounced_provider_event_receiver) =
            unbounded_channel();

        ctx.set_provider_event_sender(origin_provider_event_sender.clone());

        let id = ctx.env.provider_id.clone();
        let debounce_delay = ctx.provider_debounce();

        tokio::spawn(async move {
            let mut on_move_timer = DebounceTimer::new(Duration::from_millis(200));
            let mut on_typed_timer = DebounceTimer::new(Duration::from_millis(debounce_delay));

            // TODO: this does not fully resolve the problem, find another solution.
            // Text input from users could be overloaded in a short period of time, e.g., OnMove
            // and OnTyped can be too frequent if user types too fast, in which case we observe
            // the receiver side of unbounded channel may not be able to receive the events in time,
            // leading to the annoying frozen UI on the vim side. The cause is probably because of
            // the event processing logic on the receiver side are not running in separate tasks,
            // the processing of incoming messages cannot keep up with the rate of message generation,
            // rendering the receiver may hang for a while. One proper solution is to process each
            // provider event in a separate task, that requires more effoets however, now we choose to
            // debounce the stream to avoid overwhelming the system.
            while let Some(event) = origin_provider_event_receiver.recv().await {
                tracing::debug!("Recv origin event: {event:?}");

                let should_emit = match &event {
                    ProviderEvent::OnMove(..) => on_move_timer.should_emit(),
                    ProviderEvent::OnTyped(..) => on_typed_timer.should_emit(),
                    _ => true,
                };

                // Send event after debounce period
                if should_emit && debounced_provider_event_sender.send(event).is_err() {
                    return;
                }
            }
        });

        let provider_session = ProviderSession {
            ctx,
            id,
            provider_session_id,
            provider,
            provider_events: debounced_provider_event_receiver,
        };

        (provider_session, origin_provider_event_sender)
    }

    pub fn start_event_loop(self) {
        let debounce_delay = self.ctx.provider_debounce();

        tracing::debug!(
            provider_session_id = self.provider_session_id,
            provider_id = %self.ctx.provider_id(),
            debounce_delay,
            "Spawning a new provider session task",
        );

        tokio::spawn(async move {
            if debounce_delay > 0 {
                self.run_event_loop_with_debounce(debounce_delay).await;
            } else {
                self.run_event_loop_without_debounce().await;
            }
        });
    }

    // https://github.com/denoland/deno/blob/1fb5858009f598ce3f917f9f49c466db81f4d9b0/cli/lsp/diagnostics.rs#L141
    //
    // Debounce timer delay. 150ms between keystrokes is about 45 WPM, so we
    // want something that is longer than that, but not too long to
    // introduce detectable UI delay; 200ms is a decent compromise.
    async fn run_event_loop_with_debounce(mut self, debounce_delay: u64) {
        // If the debounce timer isn't active, it will be set to expire "never",
        // which is actually just 1 year in the future.
        const NEVER: Duration = Duration::from_secs(365 * 24 * 60 * 60);

        let mut on_move = None;
        let on_move_delay = Duration::from_millis(50);
        let on_move_timer = tokio::time::sleep(NEVER);
        tokio::pin!(on_move_timer);

        let mut on_typed = None;
        // Delay can be adjusted once we know the provider source scale.
        //
        // Here is the benchmark result of filtering on AMD 5900X:
        //
        // |    Type     |  1k   |  10k   | 100k  |
        // |    ----     |  ---- | ----   | ----  |
        // |     filter  | 413us | 12ms   | 75ms  |
        // | par_filter  | 327us |  3ms   | 20ms  |
        let mut on_typed_delay = Duration::from_millis(debounce_delay);
        let on_typed_timer = tokio::time::sleep(NEVER);
        tokio::pin!(on_typed_timer);

        loop {
            tokio::select! {
                maybe_event = self.provider_events.recv() => {
                    match maybe_event {
                        Some(event) => {
                            tracing::trace!(debounce = true, "[{}] Recv debounced event: {event:?}", self.id);

                            match event {
                                ProviderEvent::Internal(internal_event) => {
                                    match self.handle_internal_event(internal_event).await {
                                        ControlFlow::Break(_) => break,
                                        ControlFlow::Continue(maybe_new_debounce) => {
                                            if let Some(new_delay) = maybe_new_debounce {
                                                on_typed_delay = new_delay;
                                            }
                                        }
                                    }
                                }
                                ProviderEvent::OnMove(params) => {
                                    on_move.replace(params);
                                    on_move_timer.as_mut().reset(Instant::now() + on_move_delay);
                                }
                                ProviderEvent::OnTyped(params) => {
                                    on_typed.replace(params);
                                    on_typed_timer.as_mut().reset(Instant::now() + on_typed_delay);
                                }
                                ProviderEvent::Key(key_event) => {
                                    if let Err(err) = self.provider.on_key_event(&mut self.ctx, key_event).await {
                                        tracing::error!(?err, "Failed to process key_event");
                                    }
                                }
                                ProviderEvent::Exit => {
                                    self.handle_exit();
                                    return;
                                }
                                ProviderEvent::RemoteSink(params) => {
                                    self.handle_remote_sink(params).await;
                                    return;
                                }
                            }
                          }
                          None => break, // channel has closed.
                      }
                }
                _ = on_move_timer.as_mut(), if on_move.is_some() => {
                    if let Some(_params) = on_move.take() {
                        on_move_timer.as_mut().reset(Instant::now() + NEVER);

                        if let Err(err) = self.provider.on_move(&mut self.ctx).await {
                            tracing::error!(?err, "Failed to process ProviderEvent::OnMove");
                        }
                    }
                }
                _ = on_typed_timer.as_mut(), if on_typed.is_some() => {
                    if let Some(_params) = on_typed.take() {
                        on_typed_timer.as_mut().reset(Instant::now() + NEVER);

                        let _ = self.ctx.record_input().await;

                        if let Err(err) = self.provider.on_typed(&mut self.ctx).await {
                            tracing::error!(?err, "Failed to process ProviderEvent::OnTyped");
                        }

                        let _ = self.provider.on_move(&mut self.ctx).await;
                    }
                }
            }
        }
    }

    async fn run_event_loop_without_debounce(mut self) {
        while let Some(event) = self.provider_events.recv().await {
            tracing::trace!(debounce = false, "[{}] Received event: {event:?}", self.id);

            match event {
                ProviderEvent::Internal(internal_event) => {
                    if self.handle_internal_event(internal_event).await.is_break() {
                        break;
                    }
                }
                ProviderEvent::OnMove(_params) => {
                    if let Err(err) = self.provider.on_move(&mut self.ctx).await {
                        tracing::debug!(?err, "Failed to process OnMove");
                    }
                }
                ProviderEvent::OnTyped(_params) => {
                    let _ = self.ctx.record_input().await;
                    if let Err(err) = self.provider.on_typed(&mut self.ctx).await {
                        tracing::debug!(?err, "Failed to process OnTyped");
                    }
                }
                ProviderEvent::Key(key_event) => {
                    if let Err(err) = self.provider.on_key_event(&mut self.ctx, key_event).await {
                        tracing::error!(?err, "Failed to process key_event");
                    }
                }
                ProviderEvent::Exit => {
                    self.handle_exit();
                    break;
                }
                ProviderEvent::RemoteSink(params) => {
                    self.handle_remote_sink(params).await;
                    break;
                }
            }
        }
    }

    fn handle_exit(&mut self) {
        self.provider
            .on_terminate(&mut self.ctx, self.provider_session_id);
    }

    async fn handle_remote_sink(&mut self, params: Params) {
        #[derive(serde::Deserialize)]
        struct RemoteSinkParams {
            line_numbers: Vec<usize>,
        }

        let RemoteSinkParams { line_numbers } = match params.parse() {
            Ok(v) => v,
            Err(err) => {
                tracing::error!(
                    ?err,
                    "Failed to parse RemoteSink params, provider terminated"
                );
                self.provider
                    .on_terminate(&mut self.ctx, self.provider_session_id);
                return;
            }
        };

        if let Err(err) = self.provider.remote_sink(&mut self.ctx, line_numbers).await {
            tracing::error!(?err, "Failed to process remote_sink");
        }

        self.provider
            .on_terminate(&mut self.ctx, self.provider_session_id);
    }

    /// Handles the internal provider event, returns an optional new debounce delay when the
    /// control flow continues.
    async fn handle_internal_event(
        &mut self,
        internal_event: InternalProviderEvent,
    ) -> ControlFlow<(), Option<Duration>> {
        match internal_event {
            InternalProviderEvent::Terminate => {
                self.provider
                    .on_terminate(&mut self.ctx, self.provider_session_id);
                ControlFlow::Break(())
            }
            InternalProviderEvent::Initialize => {
                // Primarily initialize the provider source.
                match self.provider.on_initialize(&mut self.ctx).await {
                    Ok(()) => {
                        // Try to fulfill the preview window
                        if let Err(err) = self.provider.on_move(&mut self.ctx).await {
                            tracing::debug!(?err, "Preview after on_initialize failure");
                        }
                    }
                    Err(err) => {
                        tracing::error!(?err, "Failed to process {internal_event:?}");
                    }
                }

                // Set a smaller debounce if the source scale is small.
                let maybe_new_debounce = self.ctx.adaptive_debounce_delay();

                ControlFlow::Continue(maybe_new_debounce)
            }
            InternalProviderEvent::InitialQuery(initial_query) => {
                let _ = self
                    .provider
                    .on_initial_query(&mut self.ctx, initial_query)
                    .await;
                ControlFlow::Continue(None)
            }
        }
    }
}

#[derive(Debug)]
pub struct PluginSession {
    plugin: Box<dyn ClapPlugin>,
    plugin_events: UnboundedReceiver<PluginEvent>,
}

impl PluginSession {
    pub fn create(
        plugin: Box<dyn ClapPlugin>,
        maybe_event_delay: Option<Duration>,
    ) -> UnboundedSender<PluginEvent> {
        let (plugin_event_sender, plugin_event_receiver) = unbounded_channel();

        let plugin_session = PluginSession {
            plugin,
            plugin_events: plugin_event_receiver,
        };

        if let Some(event_delay) = maybe_event_delay {
            plugin_session.start_event_loop(event_delay);
        } else {
            plugin_session.start_event_loop_without_debounce();
        }

        plugin_event_sender
    }

    fn start_event_loop_without_debounce(mut self) {
        tracing::debug!(debounce = false, id = ?self.plugin.id(), "starting a new plugin");

        tokio::spawn(async move {
            loop {
                tokio::select! {
                  maybe_plugin_event = self.plugin_events.recv() => {
                      if let Some(plugin_event) = maybe_plugin_event {
                          let res = match plugin_event.clone() {
                              PluginEvent::Autocmd(autocmd) => self.plugin.handle_autocmd(autocmd).await,
                              PluginEvent::Action(action) => self.plugin.handle_action(action).await,
                          };
                          if let Err(err) = res {
                              tracing::error!(?err, id = self.plugin.id(), "Failed to process {plugin_event:?}");
                          }
                      } else {
                          break;
                      }
                  }
                }
            }
        });
    }

    fn start_event_loop(mut self, event_delay: Duration) {
        let id = self.plugin.id();

        tracing::debug!(debounce = ?event_delay, ?id, "starting a new plugin");

        tokio::spawn(async move {
            // If the debounce timer isn't active, it will be set to expire "never",
            // which is actually just 1 year in the future.
            const NEVER: Duration = Duration::from_secs(365 * 24 * 60 * 60);

            let mut pending_plugin_event = None;
            let notification_timer = tokio::time::sleep(NEVER);
            tokio::pin!(notification_timer);

            loop {
                tokio::select! {
                    maybe_plugin_event = self.plugin_events.recv() => {
                        match maybe_plugin_event {
                            Some(plugin_event) => {
                                // tracing::trace!(?plugin_event, "[{id}] Received event");

                                if plugin_event.should_debounce() {
                                    pending_plugin_event.replace(plugin_event);
                                    notification_timer.as_mut().reset(Instant::now() + event_delay);
                                } else {
                                    let res = match plugin_event.clone() {
                                        PluginEvent::Autocmd(autocmd) => self.plugin.handle_autocmd(autocmd).await,
                                        PluginEvent::Action(action) => self.plugin.handle_action(action).await,
                                    };
                                    if let Err(err) = res {
                                        tracing::error!(?err, id, "Failed to process {plugin_event:?}");
                                    }
                                }
                            }
                            None => break, // channel has closed.
                        }
                    }
                    _ = notification_timer.as_mut(), if pending_plugin_event.is_some() => {
                        notification_timer.as_mut().reset(Instant::now() + NEVER);

                        if let Some(autocmd) = pending_plugin_event.take() {
                            let res = match autocmd.clone() {
                                PluginEvent::Autocmd(autocmd) => self.plugin.handle_autocmd(autocmd).await,
                                PluginEvent::Action(action) => self.plugin.handle_action(action).await,
                            };
                            if let Err(err) = res {
                                tracing::error!(?err, id, "Failed to process {autocmd:?}");
                            }
                        }
                    }
                }
            }
        });
    }
}

/// This structs manages all the created sessions.
///
/// A plugin is a general service, a provider is a specialized plugin
/// which is dedicated to provide the filtering service.
#[derive(Debug, Default)]
pub struct ServiceManager {
    pub providers: HashMap<ProviderSessionId, ProviderEventSender>,
    pub plugins: HashMap<PluginId, (Vec<AutocmdEventType>, UnboundedSender<PluginEvent>)>,
}

impl ServiceManager {
    /// Creates a new provider session if `provider_session_id` does not exist.
    pub fn new_provider(
        &mut self,
        provider_session_id: ProviderSessionId,
        provider: Box<dyn ClapProvider>,
        ctx: Context,
    ) {
        // Only one provider instance is allowed.
        //
        // Kill the existing providers if any before creating a new one.
        for (provider_session_id, sender) in self.providers.drain() {
            tracing::debug!(?provider_session_id, "Sending internal Terminate signal");
            sender.send(ProviderEvent::Internal(InternalProviderEvent::Terminate));
        }

        if let Entry::Vacant(v) = self.providers.entry(provider_session_id) {
            let (provider_session, provider_event_sender) =
                ProviderSession::new(ctx, provider_session_id, provider);

            provider_session.start_event_loop();

            provider_event_sender
                .send(ProviderEvent::Internal(InternalProviderEvent::Initialize))
                .expect("Failed to send InternalProviderEvent::Initialize");

            v.insert(ProviderEventSender::new(
                provider_event_sender,
                provider_session_id,
            ));
        } else {
            tracing::error!(
                provider_session_id,
                "Skipped as given provider session already exists"
            );
        }
    }

    /// Creates a new plugin session with the default debounce setting (50ms).
    pub fn register_plugin(
        &mut self,
        plugin: Box<dyn ClapPlugin>,
        maybe_debounce: Option<Duration>,
    ) -> (PluginId, Vec<String>) {
        let plugin_id = plugin.id();

        let all_actions = plugin
            .actions(ActionType::All)
            .iter()
            .map(|s| s.method.to_string())
            .collect();

        let debounce = Some(maybe_debounce.unwrap_or(Duration::from_millis(50)));

        let subscriptions = plugin.subscriptions().to_vec();
        let plugin_event_sender = PluginSession::create(plugin, debounce);

        self.plugins
            .insert(plugin_id, (subscriptions, plugin_event_sender));

        (plugin_id, all_actions)
    }

    #[allow(unused)]
    pub fn register_plugin_without_debounce(
        &mut self,
        plugin_id: PluginId,
        plugin: Box<dyn ClapPlugin>,
    ) {
        let subscriptions = plugin.subscriptions().to_vec();
        let plugin_event_sender = PluginSession::create(plugin, None);
        self.plugins
            .insert(plugin_id, (subscriptions, plugin_event_sender));
    }

    /// Sends event message to all plugins.
    pub fn notify_plugins(&mut self, autocmd: AutocmdEvent) {
        self.plugins
            .retain(|_plugin_id, (subscriptions, plugin_sender)| {
                if subscriptions.contains(&autocmd.0) {
                    return plugin_sender
                        .send(PluginEvent::Autocmd(autocmd.clone()))
                        .is_ok();
                }
                true
            });
    }

    pub fn notify_plugin_action(&mut self, plugin_id: PluginId, plugin_action: PluginAction) {
        if let Entry::Occupied(v) = self.plugins.entry(plugin_id) {
            if v.get().1.send(PluginEvent::Action(plugin_action)).is_err() {
                tracing::error!("plugin {plugin_id} exited");
                v.remove_entry();
            }
        }
    }

    pub fn exists(&self, provider_session_id: ProviderSessionId) -> bool {
        self.providers.contains_key(&provider_session_id)
    }

    pub fn try_exit(&mut self, provider_session_id: ProviderSessionId) {
        if self.exists(provider_session_id) {
            self.notify_provider_exit(provider_session_id);
        }
    }

    /// Dispatch the session event to the background session task accordingly.
    pub fn notify_provider(&self, provider_session_id: ProviderSessionId, event: ProviderEvent) {
        if let Some(sender) = self.providers.get(&provider_session_id) {
            sender.send(event);
        } else {
            tracing::error!(
                provider_session_id,
                sessions = ?self.providers.keys(),
                "Couldn't find the sender for given session",
            );
        }
    }

    /// Stop the session task by sending [`ProviderEvent::Exit`].
    pub fn notify_provider_exit(&mut self, provider_session_id: ProviderSessionId) {
        if let Some(sender) = self.providers.remove(&provider_session_id) {
            sender.send(ProviderEvent::Exit);
        }
    }
}
