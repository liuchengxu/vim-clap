use serde::Serialize;

#[derive(Debug, Default, Clone, Serialize)]
pub struct Count {
    pub error: usize,
    pub warn: usize,
}

pub enum Direction {
    First,
    Last,
    Next,
    Prev,
}

pub enum DiagnosticKind {
    Error,
    Warn,
}
