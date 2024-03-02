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
    Sidebar,
    ClapProvider,
}
