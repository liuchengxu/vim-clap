const DEFINITION: &[&str] = &[
    "enum", "trait", "struct", "fn", "const", "static", "crate", "mod",
];

const REFERENCE: &[&str] = &["use", "impl", "let"];

const STATEMENT: &[&str] = &[
    "as", "break", "continue", "else", "extern", "false", "for", "if", "impl", "in", "let", "loop",
    "match", "move", "mut", "pub", "ref", "return", "self", "Self", "static", "super", "true",
    "type", "unsafe", "where", "while",
];

pub fn pattern_weighttoken(token: &str) -> Option<usize> {
    if DEFINITION.contains(&token) {
        Some(4)
    } else if REFERENCE.contains(&token) {
        Some(6)
    } else if token.starts_with("pub") {
        Some(7)
    } else if STATEMENT.contains(&token) {
        Some(8)
    } else if token.starts_with("[cfg") {
        Some(10)
    } else {
        None
    }
}
