use std::collections::HashMap;

use crate::stdio_server::provider::ProviderId;
use crate::stdio_server::session::SessionId;
use tokio::sync::mpsc::UnboundedSender;

#[derive(Debug, Clone)]
pub enum Event {
    Provider(ProviderEvent),
    Key(KeyEvent),
    Other(String),
}

/// Provider specific events.
#[derive(Debug, Clone)]
pub enum ProviderEvent {
    NewSession,
    /// Internal signal.
    OnInitialize,
    OnMove,
    OnTyped,
    Terminate,
    Key(KeyEvent),
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

impl Event {
    pub fn from_method(method: &str) -> Self {
        match method {
            "new_session" => Self::Provider(ProviderEvent::NewSession),
            "on_typed" => Self::Provider(ProviderEvent::OnTyped),
            "on_move" => Self::Provider(ProviderEvent::OnMove),
            "exit" => Self::Provider(ProviderEvent::Terminate),
            "cr" => Self::Key(KeyEvent::CarriageReturn),
            "tab" => Self::Key(KeyEvent::Tab),
            "backspace" => Self::Key(KeyEvent::Backspace),
            "shift-up" => Self::Key(KeyEvent::ShiftUp),
            "shift-down" => Self::Key(KeyEvent::ShiftDown),
            "ctrl-n" => Self::Key(KeyEvent::CtrlN),
            "ctrl-p" => Self::Key(KeyEvent::CtrlP),
            other => Self::Other(other.to_string()),
        }
    }
}

/// A small wrapper of `UnboundedSender<ProviderEvent>` for logging on sending error.
#[derive(Debug)]
pub struct ProviderEventSender {
    pub sender: UnboundedSender<ProviderEvent>,
    pub id: SessionId,
}

impl ProviderEventSender {
    pub fn new(sender: UnboundedSender<ProviderEvent>, id: SessionId) -> Self {
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
pub struct InputHistory(HashMap<ProviderId, Vec<String>>);

impl InputHistory {
    pub fn new() -> Self {
        Self(HashMap::new())
    }

    pub fn inputs(&self, provider_id: &ProviderId) -> Vec<String> {
        self.0.get(provider_id).cloned().unwrap_or_default()
    }

    pub fn append(&mut self, provider_id: ProviderId, mut new_inputs: Vec<String>) {
        self.0
            .entry(provider_id)
            .and_modify(|v| v.append(&mut new_inputs))
            .or_insert(new_inputs);
    }
}

#[derive(Debug, Clone)]
pub struct InputRecorder {
    pub inputs: Vec<String>,
    pub last_input: String,
    pub current_index: usize,
}

impl InputRecorder {
    pub fn new(inputs: Vec<String>) -> Self {
        Self {
            inputs,
            last_input: Default::default(),
            current_index: 0usize,
        }
    }

    pub fn into_inputs(self) -> Vec<String> {
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
        self.inputs.push(new.to_string());
        self.last_input = new.to_string();
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
