const DEFINITION: &[&str] = &["fun"];

const REFERENCE: &[&str] = &[];

const STATEMENT: &[&str] = &[
    "after", "and", "andalso", "band", "begin", "bnot", "bor", "bsl", "bsr", "bxor", "case",
    "catch", "cond", "div", "end", "if", "let", "not", "of", "or", "orelse", "receive", "rem",
    "try", "when", "xor",
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
