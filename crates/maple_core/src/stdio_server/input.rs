use crate::stdio_server::provider::ProviderId;
use crate::stdio_server::service::ProviderSessionId;
use std::collections::{HashMap, VecDeque};
use tokio::sync::mpsc::UnboundedSender;

#[derive(Debug)]
pub enum Event {
    Provider(ProviderEvent),
    Autocmd(Autocmd),
    Key(KeyEvent),
    Other(String),
}

#[derive(Debug, Clone)]
pub enum PluginEvent {
    Autocmd(Autocmd),
}

/// Provider specific events.
#[derive(Debug)]
pub enum ProviderEvent {
    NewSession,
    OnMove,
    OnTyped,
    Exit,
    Key(KeyEvent),
    /// Signal fired internally.
    Internal(InternalProviderEvent),
}

#[derive(Debug)]
pub enum InternalProviderEvent {
    OnInitialize,
    Terminate,
}

/// Represents a key event.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Hash)]
pub enum KeyEvent {
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
pub enum Autocmd {
    CursorMoved,
    InsertEnter,
}

impl Event {
    pub fn from_method(method: &str) -> Self {
        match method {
            "new_session" => Self::Provider(ProviderEvent::NewSession),
            "on_typed" => Self::Provider(ProviderEvent::OnTyped),
            "on_move" => Self::Provider(ProviderEvent::OnMove),
            "exit" => Self::Provider(ProviderEvent::Exit),
            "cr" => Self::Key(KeyEvent::CarriageReturn),
            "tab" => Self::Key(KeyEvent::Tab),
            "backspace" => Self::Key(KeyEvent::Backspace),
            "shift-up" => Self::Key(KeyEvent::ShiftUp),
            "shift-down" => Self::Key(KeyEvent::ShiftDown),
            "ctrl-n" => Self::Key(KeyEvent::CtrlN),
            "ctrl-p" => Self::Key(KeyEvent::CtrlP),
            "CursorMoved" => Self::Autocmd(Autocmd::CursorMoved),
            "InsertEnter" => Self::Autocmd(Autocmd::InsertEnter),
            other => Self::Other(other.to_string()),
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

#[derive(Debug, Clone, Default)]
pub struct InputHistory(HashMap<ProviderId, VecDeque<String>>);

impl InputHistory {
    pub fn new() -> Self {
        Self(HashMap::new())
    }

    pub fn inputs(&self, provider_id: &ProviderId) -> VecDeque<String> {
        self.0.get(provider_id).cloned().unwrap_or_default()
    }

    pub fn insert(&mut self, provider_id: ProviderId, new_value: VecDeque<String>) {
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
            .unwrap_or(self.inputs.len() - 1);
        self.inputs.get(self.current_index).map(AsRef::as_ref)
    }
}
