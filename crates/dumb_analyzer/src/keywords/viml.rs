pub fn pattern_weight(item: Option<&str>) -> Option<usize> {
    item.and_then(|s| {
        // function[!]
        if s.starts_with("function") {
            Some(3)
        } else if s == "let" {
            Some(6)
        } else {
            None
        }
    })
}
