use crate::stdio_server::provider::ProviderId;
use crate::stdio_server::service::ProviderSessionId;
use rpc::{Params, RpcNotification};
use std::collections::{HashMap, HashSet, VecDeque};
use tokio::sync::mpsc::UnboundedSender;

pub type KeyEvent = (KeyEventType, Params);
pub type AutocmdEvent = (AutocmdEventType, Params);

#[derive(Debug, Clone)]
pub enum PluginEvent {
    Autocmd(AutocmdEvent),
}

impl PluginEvent {
    /// Returns `true` if the event can be potentially too frequent and should be debounced.
    pub fn should_debounce(&self) -> bool {
        match self {
            Self::Autocmd((autocmd_event_type, _)) => match autocmd_event_type {
                AutocmdEventType::CursorMoved => true,
                _ => false,
            },
        }
    }
}

/// Provider specific events.
#[derive(Debug)]
pub enum ProviderEvent {
    NewSession(Params),
    OnMove(Params),
    OnTyped(Params),
    Exit,
    Key(KeyEvent),
    /// Signal fired internally.
    Internal(InternalProviderEvent),
}

#[derive(Debug)]
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
}

/// Represents a key event.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Hash)]
pub enum AutocmdEventType {
    CursorMoved,
    InsertEnter,
    BufEnter,
    BufWinEnter,
    BufLeave,
    BufDelete,
    BufWinLeave,
}

#[derive(Debug, Clone)]
pub struct Action {
    pub command: String,
    pub params: Params,
}

#[derive(Debug)]
pub enum Event {
    Provider(ProviderEvent),
    /// `:h autocmd`
    Autocmd(AutocmdEvent),
    /// `:h keycodes`
    Key(KeyEvent),
    /// User-oriented actions.
    Action(Action),
}

impl Event {
    /// Converts the notification to an [`Event`].
    pub fn parse_notification(notification: RpcNotification) -> Self {
        match notification.method.as_str() {
            "exit" => Self::Provider(ProviderEvent::Exit),
            "on_move" => Self::Provider(ProviderEvent::OnMove(notification.params)),
            "on_typed" => Self::Provider(ProviderEvent::OnTyped(notification.params)),
            "new_session" => Self::Provider(ProviderEvent::NewSession(notification.params)),
            "cr" => Self::Key((KeyEventType::CarriageReturn, notification.params)),
            "tab" => Self::Key((KeyEventType::Tab, notification.params)),
            "ctrl-n" => Self::Key((KeyEventType::CtrlN, notification.params)),
            "ctrl-p" => Self::Key((KeyEventType::CtrlP, notification.params)),
            "shift-up" => Self::Key((KeyEventType::ShiftUp, notification.params)),
            "shift-down" => Self::Key((KeyEventType::ShiftDown, notification.params)),
            "backspace" => Self::Key((KeyEventType::Backspace, notification.params)),
            "CursorMoved" => Self::Autocmd((AutocmdEventType::CursorMoved, notification.params)),
            "InsertEnter" => Self::Autocmd((AutocmdEventType::InsertEnter, notification.params)),
            "BufEnter" => Self::Autocmd((AutocmdEventType::BufEnter, notification.params)),
            "BufLeave" => Self::Autocmd((AutocmdEventType::BufLeave, notification.params)),
            "BufDelete" => Self::Autocmd((AutocmdEventType::BufDelete, notification.params)),
            "BufWinEnter" => Self::Autocmd((AutocmdEventType::BufWinEnter, notification.params)),
            "BufWinLeave" => Self::Autocmd((AutocmdEventType::BufWinLeave, notification.params)),
            _ => Self::Action(Action {
                command: notification.method,
                params: notification.params,
            }),
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
#[derive(Debug, Clone, Default)]
pub struct InputHistory(HashMap<ProviderId, VecDeque<String>>);

impl InputHistory {
    pub fn new() -> Self {
        Self(HashMap::new())
    }

    pub fn inputs(&self, provider_id: &ProviderId) -> VecDeque<String> {
        self.0.get(provider_id).cloned().unwrap_or_default()
    }

    pub fn all_inputs(&self) -> VecDeque<String> {
        // HashSet gurantees no duplicated elements.
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

    pub fn try_record(&mut self, new: String) {
        let new = new.trim();

        if new.is_empty() || self.inputs.iter().any(|s| s == new) {
            return;
        }

        // New input is part of some old input.
        if self.inputs.iter().any(|old| old.starts_with(new)) {
            return;
        }

        // Prune the last input if the consecutive input is extending it.
        // Avoid recording the partial incomplete list, e.g., `i, in, inp, inpu, input`.
        if new.starts_with(&self.last_input) {
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
        self.inputs.push_back(new.to_string());
        self.last_input = new.to_string();

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
    pub fn move_to_previous(&mut self) -> Option<&str> {
        if self.inputs.is_empty() {
            return None;
        }
        self.current_index = self
            .current_index
            .checked_sub(1)
            .unwrap_or_else(|| self.inputs.len() - 1);
        self.inputs.get(self.current_index).map(AsRef::as_ref)
    }
}
