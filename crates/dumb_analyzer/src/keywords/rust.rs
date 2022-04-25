use super::KeywordPriority;

pub struct Rust;

impl KeywordPriority for Rust {
    const DEFINITION: &'static [&'static str] = &[
        "enum", "trait", "struct", "fn", "const", "static", "crate", "mod",
    ];

    const REFERENCE: &'static [&'static str] = &["use", "impl", "let"];

    const STATEMENT: &'static [&'static str] = &[
        "as", "break", "continue", "else", "extern", "false", "for", "if", "impl", "in", "let",
        "loop", "match", "move", "mut", "pub", "ref", "return", "self", "Self", "static", "super",
        "true", "type", "unsafe", "where", "while",
    ];

    fn keyword_priority(token: &str) -> Option<usize> {
        if Self::DEFINITION.contains(&token) {
            Some(4)
        } else if Self::REFERENCE.contains(&token) {
            Some(6)
        } else if token.starts_with("pub") {
            Some(7)
        } else if Self::STATEMENT.contains(&token) {
            Some(8)
        } else if token.starts_with("[cfg") {
            Some(10)
        } else {
            None
        }
    }
}
