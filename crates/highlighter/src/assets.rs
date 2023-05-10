use syntect::dumps;
use syntect::highlighting::ThemeSet;
use syntect::parsing::SyntaxSet;

pub const DEFAULT_SYNTAXSET: &[u8] = include_bytes!("../../../assets/syntaxes.bin");
pub const DEFAULT_THEMESET: &[u8] = include_bytes!("../../../assets/themes.bin");

pub struct HighlightingAssets {
    pub syntax_set: SyntaxSet,
    pub theme_set: ThemeSet,
}

impl HighlightingAssets {
    pub fn new() -> Self {
        Self {
            syntax_set: dumps::from_binary(DEFAULT_SYNTAXSET),
            theme_set: dumps::from_binary(DEFAULT_THEMESET),
        }
    }
}
