use std::collections::HashMap;

use once_cell::sync::OnceCell;

fn rs_kind_alias() -> HashMap<&'static str, &'static str> {
    vec![
        ("module", "mod"),
        ("typedef", "type"),
        ("function", "fn"),
        ("interface", "trait"),
        ("enumerator", "enum"),
        ("implementation", "impl"),
    ]
    .into_iter()
    .collect()
}

fn get_kind_alias<'a>(extension: &'a str, kind: &'a str) -> Option<&'a &'static str> {
    static KIND_MAP: OnceCell<HashMap<&str, HashMap<&str, &str>>> = OnceCell::new();

    let map = KIND_MAP.get_or_init(|| {
        vec![("rs", rs_kind_alias())]
            .into_iter()
            .collect::<HashMap<_, _>>()
    });

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
