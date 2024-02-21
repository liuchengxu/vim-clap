use super::KeywordPriority;

pub struct Erlang;

impl KeywordPriority for Erlang {
    const DEFINITION: &'static [&'static str] = &["fun"];

    const REFERENCE: &'static [&'static str] = &[];

    const STATEMENT: &'static [&'static str] = &[
        "after", "and", "andalso", "band", "begin", "bnot", "bor", "bsl", "bsr", "bxor", "case",
        "catch", "cond", "div", "end", "if", "let", "not", "of", "or", "orelse", "receive", "rem",
        "try", "when", "xor",
    ];
}
