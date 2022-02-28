use super::KeywordWeight;

pub struct Go;

impl KeywordWeight for Go {
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
