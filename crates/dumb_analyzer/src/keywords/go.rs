const DEFINITION: &[&str] = &[
    "enum",
    "interface",
    "struct",
    "func",
    "const",
    "type",
    "package",
];

const REFERENCE: &[&str] = &["import"];

const STATEMENT: &[&str] = &[
    "break",
    "case",
    "chan",
    "continue",
    "default",
    "defer",
    "else",
    "fallthrough",
    "for",
    "go",
    "goto",
    "if",
    "map",
    "range",
    "return",
    "select",
    "switch",
    "var",
];

pub fn token_weight(token: &str) -> Option<usize> {
    if DEFINITION.contains(&token) {
        Some(4)
    } else if REFERENCE.contains(&token) {
        Some(6)
    } else if STATEMENT.contains(&token) {
        Some(8)
    } else {
        None
    }
}
