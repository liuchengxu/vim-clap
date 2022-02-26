const TYPE: &[&str] = &["type", "mod", "impl"];
const FUNCTION: &[&str] = &["fn", "macro_rules"];
const VARIABLE: &[&str] = &["let", "const", "static", "enum", "struct", "trait"];

pub fn pattern_weight(token: &str) -> Option<usize> {
    if FUNCTION.contains(&token) {
        Some(4)
    } else if token.starts_with("pub") {
        Some(6)
    } else if TYPE.contains(&token) {
        Some(7)
    } else if VARIABLE.contains(&token) {
        Some(8)
    } else if token.starts_with("[cfg(feature") {
        Some(10)
    } else {
        None
    }
}
