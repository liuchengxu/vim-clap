pub fn token_weight(token: &str) -> Option<usize> {
    // function[!]
    if token.starts_with("function") {
        Some(3)
    } else if token == "let" {
        Some(6)
    } else {
        None
    }
}
