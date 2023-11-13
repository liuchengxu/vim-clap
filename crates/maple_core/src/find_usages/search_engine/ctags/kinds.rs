use once_cell::sync::OnceCell;
use std::collections::HashMap;

fn rs_kind_alias() -> HashMap<&'static str, &'static str> {
    HashMap::from([
        ("module", "mod"),
        ("typedef", "type"),
        ("function", "fn"),
        ("interface", "trait"),
        ("enumerator", "enum"),
        ("implementation", "impl"),
    ])
}

fn get_kind_alias<'a>(extension: &'a str, kind: &'a str) -> Option<&'a &'static str> {
    static KIND_MAP: OnceCell<HashMap<&str, HashMap<&str, &str>>> = OnceCell::new();

    let map = KIND_MAP.get_or_init(|| HashMap::from([("rs", rs_kind_alias())]));

    map.get(extension).and_then(|m| m.get(kind))
}

/// Returns the compact kind given the original form.
///
/// Make the kind field shorter to save more spaces for the other fields.
pub fn compact_kind(maybe_extension: Option<&str>, kind: &str) -> String {
    maybe_extension
        .and_then(|extension| get_kind_alias(extension, kind))
        .unwrap_or(&kind)
        .to_string()
}
