const TYPE: &[&str] = &["type", "mod", "impl"];
const FUNCTION: &[&str] = &["fn", "macro_rules"];
const VARIABLE: &[&str] = &["let", "const", "static", "enum", "struct", "trait"];

pub fn pattern_weight(item: Option<&str>) -> Option<usize> {
    item.and_then(|s| {
        if FUNCTION.contains(&s) {
            Some(4)
        } else if s.starts_with("pub") {
            Some(6)
        } else if TYPE.contains(&s) {
            Some(7)
        } else if VARIABLE.contains(&s) {
            Some(8)
        } else if s.starts_with("[cfg(feature") {
            Some(10)
        } else {
            None
        }
    })
}
