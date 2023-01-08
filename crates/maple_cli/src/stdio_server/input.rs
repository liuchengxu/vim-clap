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
    Create,
    OnMove,
    OnTyped,
    Terminate,
    Key(KeyEvent),
}

/// Represents a key event.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Hash)]
pub enum KeyEvent {
    Tab,
    Backspace,
    // <CR>/<Enter>/<Return> was typed.
    CarriageReturn,
    // <S-Up>
    ShiftUp,
    // <S-Down>
    ShiftDown,
}

impl Event {
    pub fn from_method(method: &str) -> Self {
        match method {
            "new_session" => Self::Provider(ProviderEvent::Create),
            "on_typed" => Self::Provider(ProviderEvent::OnTyped),
            "on_move" => Self::Provider(ProviderEvent::OnMove),
            "exit" => Self::Provider(ProviderEvent::Terminate),
            "cr" => Self::Key(KeyEvent::CarriageReturn),
            "tab" => Self::Key(KeyEvent::Tab),
            "backspace" => Self::Key(KeyEvent::Backspace),
            "shift-up" => Self::Key(KeyEvent::ShiftUp),
            "shift-down" => Self::Key(KeyEvent::ShiftDown),
            other => Self::Other(other.to_string()),
        }
    }
}

/// A small wrapper of Sender<ProviderEvent> for logging on sending error.
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
