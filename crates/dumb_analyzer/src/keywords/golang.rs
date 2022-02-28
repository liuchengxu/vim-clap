use super::KeywordWeight;

pub struct Golang;

impl KeywordWeight for Golang {
    const DEFINITION: &'static [&'static str] = &[
        "enum",
        "interface",
        "struct",
        "func",
        "const",
        "type",
        "package",
    ];

    const REFERENCE: &'static [&'static str] = &["import"];

    const STATEMENT: &'static [&'static str] = &[
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
}
