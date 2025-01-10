use crate::stdio_server::plugin::PluginId;
use crate::stdio_server::provider::ProviderId;
use crate::stdio_server::service::ProviderSessionId;
use crate::stdio_server::Error;
use rpc::{Params, RpcNotification};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};
use tokio::sync::mpsc::UnboundedSender;

pub use types::AutocmdEventType;

pub type KeyEvent = (KeyEventType, Params);
pub type AutocmdEvent = (AutocmdEventType, Params);

#[derive(Debug, Clone)]
pub enum PluginEvent {
    Autocmd(AutocmdEvent),
    Action(PluginAction),
}

impl PluginEvent {
    /// Returns `true` if the event can be potentially too frequent and should be debounced.
    pub fn should_debounce(&self) -> bool {
        match self {
            Self::Autocmd((autocmd_event_type, _)) => {
                matches!(
                    autocmd_event_type,
                    AutocmdEventType::CursorMoved
                        | AutocmdEventType::TextChanged
                        | AutocmdEventType::TextChangedI
                )
            }
            _ => false,
        }
    }
}

/// Provider specific events.
#[derive(Debug, Clone)]
pub enum ProviderEvent {
    OnMove(Params),
    OnTyped(Params),
    RemoteSink(Params),
    Exit,
    Key(KeyEvent),
    /// Signal fired internally.
    Internal(InternalProviderEvent),
}

impl ProviderEvent {
    pub fn is_same_type(&self, other: &Self) -> bool {
        match self {
            Self::OnMove(_) => matches!(other, Self::OnMove(_)),
            Self::OnTyped(_) => matches!(other, Self::OnTyped(_)),
            Self::RemoteSink(_) => matches!(other, Self::RemoteSink(_)),
            Self::Exit => matches!(other, Self::Exit),
            Self::Key(_) => matches!(other, Self::Key(_)),
            Self::Internal(_) => matches!(other, Self::Internal(_)),
        }
    }
}

#[derive(Debug, Clone)]
pub enum InternalProviderEvent {
    Initialize,
    InitialQuery(String),
    Terminate,
}

/// Represents a key event type.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Hash)]
pub enum KeyEventType {
    // Ctrl-I/<Tab>
    Tab,
    // Ctrl-H/<BS>
    Backspace,
    // <CR>/<Enter>/<Return>
    CarriageReturn,
    // <S-Up>
    ShiftUp,
    // <S-Down>
    ShiftDown,
    // <C-N>
    CtrlN,
    // <C-P>
    CtrlP,
    // <C-t>
    CtrlT,
    // <C-x>
    CtrlX,
    // <C-v>
    CtrlV,
}

pub type ActionEvent = (PluginId, PluginAction);

#[derive(Debug, Clone)]
pub struct PluginAction {
    pub method: String,
    pub params: Params,
}

impl From<RpcNotification> for PluginAction {
    fn from(notification: RpcNotification) -> Self {
        Self {
            method: notification.method,
            params: notification.params,
        }
    }
}

#[derive(Debug)]
pub enum Event {
    NewProvider(Params),
    ProviderWorker(ProviderEvent),
    /// `:h autocmd`
    Autocmd(AutocmdEvent),
    /// `:h keycodes`
    Key(KeyEvent),
    /// Plugin actions.
    Action(ActionEvent),
}

impl Event {
    /// Converts the notification to an [`Event`].
    pub fn parse_notification(
        notification: RpcNotification,
        parse_action: impl Fn(RpcNotification) -> Result<ActionEvent, Error>,
    ) -> Result<Self, Error> {
        use KeyEventType::*;

        match notification.method.as_str() {
            "new_provider" => Ok(Self::NewProvider(notification.params)),
            "exit_provider" => Ok(Self::ProviderWorker(ProviderEvent::Exit)),
            "remote_sink" => Ok(Self::ProviderWorker(ProviderEvent::RemoteSink(
                notification.params,
            ))),
            "on_move" => Ok(Self::ProviderWorker(ProviderEvent::OnMove(
                notification.params,
            ))),
            "on_typed" => Ok(Self::ProviderWorker(ProviderEvent::OnTyped(
                notification.params,
            ))),
            "cr" => Ok(Self::Key((CarriageReturn, notification.params))),
            "tab" => Ok(Self::Key((Tab, notification.params))),
            "ctrl-n" => Ok(Self::Key((CtrlN, notification.params))),
            "ctrl-p" => Ok(Self::Key((CtrlP, notification.params))),
            "ctrl-t" => Ok(Self::Key((CtrlT, notification.params))),
            "ctrl-x" => Ok(Self::Key((CtrlX, notification.params))),
            "ctrl-v" => Ok(Self::Key((CtrlV, notification.params))),
            "shift-up" => Ok(Self::Key((ShiftUp, notification.params))),
            "shift-down" => Ok(Self::Key((ShiftDown, notification.params))),
            "backspace" => Ok(Self::Key((Backspace, notification.params))),
            autocmd_or_action => match AutocmdEventType::parse(autocmd_or_action) {
                Some(autocmd_event_type) => {
                    Ok(Self::Autocmd((autocmd_event_type, notification.params)))
                }
                None => Ok(Self::Action(parse_action(notification)?)),
            },
        }
    }
}

/// A small wrapper of `UnboundedSender<ProviderEvent>` for logging on sending error.
#[derive(Debug)]
pub struct ProviderEventSender {
    pub sender: UnboundedSender<ProviderEvent>,
    pub id: ProviderSessionId,
}

impl ProviderEventSender {
    pub fn new(sender: UnboundedSender<ProviderEvent>, id: ProviderSessionId) -> Self {
        Self { sender, id }
    }
}

impl std::fmt::Display for ProviderEventSender {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ProviderEventSender for session {}", self.id)
    }
}

impl ProviderEventSender {
    pub fn send(&self, event: ProviderEvent) {
        if let Err(error) = self.sender.send(event) {
            tracing::error!(?error, "Failed to send session event");
        }
    }
}

/// Input history of all providers.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct InputHistory(HashMap<ProviderId, VecDeque<String>>);

impl InputHistory {
    pub fn new() -> Self {
        Self(HashMap::new())
    }

    pub fn inputs(&self, provider_id: &ProviderId) -> VecDeque<String> {
        self.0.get(provider_id).cloned().unwrap_or_default()
    }

    pub fn all_inputs(&self) -> VecDeque<String> {
        // HashSet guarantees no duplicated elements.
        self.0
            .values()
            .flatten()
            .cloned()
            .collect::<HashSet<_>>()
            .into_iter()
            .collect()
    }

    pub fn update_inputs(&mut self, provider_id: ProviderId, new_value: VecDeque<String>) {
        self.0.insert(provider_id, new_value);
    }
}

#[derive(Debug, Clone)]
pub struct InputRecorder {
    pub inputs: VecDeque<String>,
    pub last_input: String,
    pub current_index: usize,
}

impl InputRecorder {
    /// Maximum size of inputs per provider.
    const MAX_INPUTS: usize = 20;

    pub fn new(inputs: VecDeque<String>) -> Self {
        Self {
            inputs,
            last_input: Default::default(),
            current_index: 0usize,
        }
    }

    pub fn into_inputs(self) -> VecDeque<String> {
        self.inputs
    }

    pub fn record_input(&mut self, new: String) {
        let trimmed_new = new.trim();

        if trimmed_new.is_empty() || self.inputs.iter().any(|s| s == trimmed_new) {
            return;
        }

        // trimmed_new input is part of some old input.
        if self.inputs.iter().any(|old| old.starts_with(trimmed_new)) {
            return;
        }

        // Prune the last input if the consecutive input is extending it.
        // Avoid recording the partial incomplete list, e.g., `i, in, inp, inpu, input`.
        if trimmed_new.starts_with(&self.last_input) {
            if let Some(pos) = self
                .inputs
                .iter()
                .position(|i| i.as_str() == self.last_input.as_str())
            {
                if self.current_index >= pos {
                    self.current_index = self.current_index.saturating_sub(1);
                }
                self.inputs.remove(pos);
            }
        }

        if !self.inputs.is_empty() {
            self.current_index += 1;
        }
        self.inputs.push_back(new.clone());
        self.last_input = new;

        if self.inputs.len() > Self::MAX_INPUTS {
            self.inputs.pop_front();
        }
    }

    /// Returns the next input if inputs are not empty.
    ///
    /// Returns the first input if current input is the last.
    pub fn move_to_next(&mut self) -> Option<&str> {
        if self.inputs.is_empty() {
            return None;
        }
        self.current_index = (self.current_index + 1) % self.inputs.len();
        self.inputs.get(self.current_index).map(AsRef::as_ref)
    }

    /// Returns the previous input if inputs are not empty.
    ///
    /// Returns the last input if current input is the first.
    pub fn move_to_prev(&mut self) -> Option<&str> {
        if self.inputs.is_empty() {
            return None;
        }
        self.current_index = self
            .current_index
            .checked_sub(1)
            .unwrap_or(self.inputs.len() - 1);
        self.inputs.get(self.current_index).map(AsRef::as_ref)
    }
}
