use super::KeywordWeight;

pub struct Viml;

impl KeywordWeight for Viml {
    const DEFINITION: &'static [&'static str] =
        &["function", "function!", "command", "command!", "cmd"];

    const REFERENCE: &'static [&'static str] = &["let"];

    const STATEMENT: &'static [&'static str] = &[
        "for", "endfor", "while", "endwhile", "if", "elseif", "else", "endif", "call", "in",
    ];
}
