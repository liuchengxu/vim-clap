pub enum Direction {
    First,
    Last,
    Next,
    Prev,
}

pub enum DiagnosticKind {
    All,
    Error,
    Warn,
    Hint,
}

#[derive(Clone, Copy, Debug)]
pub enum Goto {
    Definition,
    Declaration,
    TypeDefinition,
    Implementation,
    Reference,
}

#[allow(dead_code)]
pub enum GotoLocationsUI {
    Quickfix,
    ClapProvider,
}
