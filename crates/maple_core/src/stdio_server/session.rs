//! Each invocation of Clap provider is a session. When you exit the provider, the session ends.

use crate::stdio_server::input::{ProviderEvent, ProviderEventSender};
use crate::stdio_server::provider::{ClapProvider, Context, ProviderSource};
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::fmt::Debug;
use std::time::Duration;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};
use tokio::sync::oneshot;
use tokio::time::Instant;

pub type SessionId = u64;

#[derive(Debug)]
pub struct Session {
    ctx: Context,
    session_id: SessionId,
    /// Each provider session can have its own message processing logic.
    provider: Box<dyn ClapProvider>,
    event_recv: UnboundedReceiver<ProviderEvent>,
}

impl Session {
    pub fn new(
        session_id: u64,
        ctx: Context,
        provider: Box<dyn ClapProvider>,
    ) -> (Self, UnboundedSender<ProviderEvent>) {
        let (session_sender, session_receiver) = unbounded_channel();

        let session = Session {
            ctx,
            session_id,
            provider,
            event_recv: session_receiver,
        };

        (session, session_sender)
    }

    pub fn start_event_loop(self) {
        tracing::debug!(
            session_id = self.session_id,
            provider_id = %self.ctx.provider_id(),
            debounce = self.ctx.env.debounce,
            "Spawning a new session task",
        );

        tokio::spawn(async move {
            if self.ctx.env.debounce {
                self.run_event_loop_with_debounce().await;
            } else {
                self.run_event_loop_without_debounce().await;
            }
        });
    }

    async fn run_event_loop_with_debounce(mut self) {
        // https://github.com/denoland/deno/blob/1fb5858009f598ce3f917f9f49c466db81f4d9b0/cli/lsp/diagnostics.rs#L141
        //
        // Debounce timer delay. 150ms between keystrokes is about 45 WPM, so we
        // want something that is longer than that, but not too long to
        // introduce detectable UI delay; 200ms is a decent compromise.
        const DELAY: Duration = Duration::from_millis(200);
        // If the debounce timer isn't active, it will be set to expire "never",
        // which is actually just 1 year in the future.
        const NEVER: Duration = Duration::from_secs(365 * 24 * 60 * 60);

        let mut on_move_dirty = false;
        let on_move_delay = Duration::from_millis(50);
        let on_move_timer = tokio::time::sleep(NEVER);
        tokio::pin!(on_move_timer);

        let mut on_typed_dirty = false;
        // Delay can be adjusted once we know the provider source scale.
        //
        // Here is the benchmark result of filtering on AMD 5900X:
        //
        // |    Type     |  1k   |  10k   | 100k  |
        // |    ----     |  ---- | ----   | ----  |
        // |     filter  | 413us | 12ms   | 75ms  |
        // | par_filter  | 327us |  3ms   | 20ms  |
        let mut on_typed_delay = DELAY;
        let on_typed_timer = tokio::time::sleep(NEVER);
        tokio::pin!(on_typed_timer);

        loop {
            tokio::select! {
                maybe_event = self.event_recv.recv() => {
                    match maybe_event {
                        Some(event) => {
                            tracing::trace!("[with_debounce] Received event: {event:?}");

                            match event {
                                ProviderEvent::NewSession => unreachable!(),
                                ProviderEvent::ForceTerminate(sender) => {
                                    self.provider.on_terminate(&mut self.ctx, self.session_id);
                                    let _ = sender.send(());
                                    break;
                                }
                                ProviderEvent::Terminate => {
                                    self.provider.on_terminate(&mut self.ctx, self.session_id);
                                    break;
                                }
                                ProviderEvent::OnInitialize => {
                                    match self.provider.on_initialize(&mut self.ctx).await {
                                        Ok(()) => {
                                            // Set a smaller debounce if the source scale is small.
                                            if let ProviderSource::Small { total, .. } = *self
                                                .ctx
                                                .provider_source
                                                .read()
                                            {
                                                if total < 10_000 {
                                                    on_typed_delay = Duration::from_millis(10);
                                                } else if total < 100_000 {
                                                    on_typed_delay = Duration::from_millis(50);
                                                } else if total < 200_000 {
                                                    on_typed_delay = Duration::from_millis(100);
                                                }
                                            }
                                            // Try to fulfill the preview window
                                            if let Err(err) = self.provider.on_move(&mut self.ctx).await {
                                                tracing::debug!(?err, "Failed to preview after on_initialize completed");
                                            }
                                        }
                                        Err(err) => {
                                            tracing::error!(?err, "Failed to process {event:?}");
                                        }
                                    }
                                }
                                ProviderEvent::OnMove => {
                                    on_move_dirty = true;
                                    on_move_timer.as_mut().reset(Instant::now() + on_move_delay);
                                }
                                ProviderEvent::OnTyped => {
                                    on_typed_dirty = true;
                                    on_typed_timer.as_mut().reset(Instant::now() + on_typed_delay);
                                }
                                ProviderEvent::Key(key_event) => {
                                    if let Err(err) = self.provider.on_key_event(&mut self.ctx, key_event).await {
                                        tracing::error!(?err, "Failed to process {event:?}");
                                    }
                                }
                            }
                          }
                          None => break, // channel has closed.
                      }
                }
                _ = on_move_timer.as_mut(), if on_move_dirty => {
                    on_move_dirty = false;
                    on_move_timer.as_mut().reset(Instant::now() + NEVER);

                    if let Err(err) = self.provider.on_move(&mut self.ctx).await {
                        tracing::error!(?err, "Failed to process ProviderEvent::OnMove");
                    }
                }
                _ = on_typed_timer.as_mut(), if on_typed_dirty => {
                    on_typed_dirty = false;
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

    async fn run_event_loop_without_debounce(mut self) {
        while let Some(event) = self.event_recv.recv().await {
            tracing::trace!("[without_debounce] Received event: {event:?}");

            match event {
                ProviderEvent::NewSession => unreachable!(),
                ProviderEvent::ForceTerminate(sender) => {
                    self.provider.on_terminate(&mut self.ctx, self.session_id);
                    let _ = sender.send(());
                    break;
                }
                ProviderEvent::Terminate => {
                    self.provider.on_terminate(&mut self.ctx, self.session_id);
                    break;
                }
                ProviderEvent::OnInitialize => {
                    if let Err(err) = self.provider.on_initialize(&mut self.ctx).await {
                        tracing::error!(?err, "Failed at process {event:?}");
                        continue;
                    }
                    // Try to fulfill the preview window
                    if let Err(err) = self.provider.on_move(&mut self.ctx).await {
                        tracing::debug!(?err, "Failed to preview after on_initialize completed");
                    }
                }
                ProviderEvent::OnMove => {
                    if let Err(err) = self.provider.on_move(&mut self.ctx).await {
                        tracing::debug!(?err, "Failed to process {event:?}");
                    }
                }
                ProviderEvent::OnTyped => {
                    let _ = self.ctx.record_input().await;
                    if let Err(err) = self.provider.on_typed(&mut self.ctx).await {
                        tracing::debug!(?err, "Failed to process {event:?}");
                    }
                }
                ProviderEvent::Key(key_event) => {
                    if let Err(err) = self.provider.on_key_event(&mut self.ctx, key_event).await {
                        tracing::error!(?err, "Failed to process {key_event:?}");
                    }
                }
            }
        }
    }
}

/// This structs manages all the created sessions tracked by the session id.
#[derive(Debug, Default)]
pub struct SessionManager {
    sessions: HashMap<SessionId, ProviderEventSender>,
}

impl SessionManager {
    /// Creates a new session if session_id does not exist.
    pub async fn new_session(
        &mut self,
        session_id: SessionId,
        provider: Box<dyn ClapProvider>,
        ctx: Context,
    ) {
        for (session_id, sender) in self.sessions.drain() {
            let (tx, rx) = oneshot::channel();
            tracing::debug!("Force terminate session {session_id} internally");
            sender.send(ProviderEvent::ForceTerminate(tx));
            sender.send(ProviderEvent::Terminate);
            let _ = rx.await;
        }

        if let Entry::Vacant(v) = self.sessions.entry(session_id) {
            let (session, session_sender) = Session::new(session_id, ctx, provider);
            session.start_event_loop();

            session_sender
                .send(ProviderEvent::OnInitialize)
                .expect("Failed to send ProviderEvent::OnInitialize");

            v.insert(ProviderEventSender::new(session_sender, session_id));
        } else {
            tracing::error!(session_id, "Skipped as given session already exists");
        }
    }

    pub fn exists(&self, session_id: SessionId) -> bool {
        self.sessions.contains_key(&session_id)
    }

    /// Stop the session task by sending [`ProviderEvent::Terminate`].
    pub fn terminate(&mut self, session_id: SessionId) {
        if let Some(sender) = self.sessions.remove(&session_id) {
            sender.send(ProviderEvent::Terminate);
        }
    }

    /// Dispatch the session event to the background session task accordingly.
    pub fn send(&self, session_id: SessionId, event: ProviderEvent) {
        if let Some(sender) = self.sessions.get(&session_id) {
            sender.send(event);
        } else {
            tracing::error!(
                session_id,
                sessions = ?self.sessions.keys(),
                "Couldn't find the sender for given session",
            );
        }
    }
}
