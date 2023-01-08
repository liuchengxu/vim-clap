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
