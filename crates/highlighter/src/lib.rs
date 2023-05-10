mod assets;
mod lazy_theme_set;

use self::lazy_theme_set::LazyThemeSet;
use anyhow::Result;
use colorsys::Rgb;
use rgb2ansi256::rgb_to_ansi256;
use std::ops::Range;
use syntect::easy::HighlightLines;
use syntect::highlighting::{FontStyle, Style, ThemeSet};
use syntect::parsing::{SyntaxReference, SyntaxSet};

/// Vim highlight arguments.
///
/// `:h highlight-args`.
#[derive(Debug)]
pub struct HighlightArgs {
    /// `:h attr-list`
    pub cterm: AttrList,
    pub ctermfg: u8,
    pub ctermbg: u8,
    pub gui: AttrList,
    pub guifg: Rgb,
    pub guibg: Rgb,
}

impl HighlightArgs {
    pub fn from_style(style: &Style) -> Self {
        let cterm = match style.font_style {
            FontStyle::BOLD => AttrList::Bold,
            FontStyle::UNDERLINE => AttrList::Underline,
            FontStyle::ITALIC => AttrList::Italic,
            _ => AttrList::None,
        };
        let guifg = Rgb::from(&(
            style.foreground.r as f32,
            style.foreground.g as f32,
            style.foreground.b as f32,
            style.foreground.a as f32,
        ));

        let ctermfg = rgb_to_ansi256(style.foreground.r, style.foreground.g, style.foreground.b);

        let gui = cterm.clone();
        let guibg = Rgb::from(&(
            style.background.r as f32,
            style.background.g as f32,
            style.background.b as f32,
            style.background.a as f32,
        ));
        let ctermbg = rgb_to_ansi256(style.background.r, style.background.g, style.background.b);

        Self {
            cterm,
            ctermfg,
            ctermbg,
            gui,
            guifg,
            guibg,
        }
    }
}

/// `:h attr-list`
#[derive(Clone, Debug, Default, serde::Serialize)]
pub enum AttrList {
    Bold,
    Underline,
    Italic,
    #[default]
    None,
}

#[derive(Debug, serde::Serialize)]
pub struct TokenHighlight {
    pub cterm: AttrList,
    pub ctermfg: u8,
    pub ctermbg: u8,
    pub gui: AttrList,
    pub guifg: String,
    pub guibg: String,
    pub group_name: String,
    /// Token range in bytes.
    pub range: Vec<usize>,
}

#[derive(Debug)]
pub struct HighlightToken {
    pub highlight_args: HighlightArgs,
    /// Token range in chars.
    pub range: Range<usize>,
}

#[derive(Debug)]
pub struct SyntaxHighlighter {
    pub syntax_set: SyntaxSet,
    pub theme_set: ThemeSet,
}

fn get_serialized_integrated_syntaxset() -> &'static [u8] {
    include_bytes!("../../../assets/syntaxes.bin")
}

fn get_integrated_themeset() -> LazyThemeSet {
    from_binary(include_bytes!("../../../assets/themes.bin"))
}

fn from_binary<T: serde::de::DeserializeOwned>(v: &[u8]) -> T {
    asset_from_contents(v, "n/a")
        .expect("data integrated in binary is never faulty, but make sure `compressed` is in sync!")
}

fn asset_from_contents<T: serde::de::DeserializeOwned>(
    contents: &[u8],
    description: &str,
) -> Result<T> {
    bincode::deserialize_from(contents)
        .map_err(|_| anyhow::anyhow!("Could not parse {description}"))
}

impl SyntaxHighlighter {
    // Load these once at the start of your program
    pub fn new() -> Self {
        Self {
            syntax_set: syntect::dumps::from_binary(crate::assets::DEFAULT_SYNTAXSET),
            theme_set: syntect::dumps::from_binary(crate::assets::DEFAULT_THEMESET),
            // syntax_set: SyntaxSet::load_defaults_newlines(),
            // theme_set: ThemeSet::load_defaults(),
        }
    }

    pub fn get_line_highlights(
        &self,
        syntax: &SyntaxReference,
        line: &str,
    ) -> Result<Vec<TokenHighlight>> {
        let mut h = HighlightLines::new(syntax, &self.theme_set.themes["Solarized (dark)"]);

        let ranges: Vec<(Style, &str)> = h.highlight_line(line, &self.syntax_set)?;

        let mut offset = 0;
        let vim_highlights = ranges
            .iter()
            .filter_map(|(style, text)| {
                let chars_count = text.chars().count();
                offset += chars_count;
                if text.trim().is_empty() {
                    None
                } else {
                    let char_indices = (offset - chars_count..offset).collect::<Vec<_>>();
                    let byte_indices = utils::char_indices_to_byte_indices(line, &char_indices);
                    let highlight_args = HighlightArgs::from_style(style);
                    let hex_guifg = highlight_args.guifg.to_hex_string();
                    let hex_guibg = highlight_args.guibg.to_hex_string();
                    let group_name: String =
                        format!("ClapHighlighter_{}_{}", &hex_guifg[1..], &hex_guifg[1..]);
                    Some(TokenHighlight {
                        cterm: highlight_args.cterm,
                        ctermfg: highlight_args.ctermfg,
                        ctermbg: highlight_args.ctermbg,
                        gui: highlight_args.gui,
                        guifg: hex_guifg,
                        guibg: hex_guibg,
                        group_name,
                        range: byte_indices,
                    })
                }
            })
            .collect::<Vec<_>>();

        Ok(vim_highlights)
    }

    pub fn highlight_line(&self, extension: &str, line: &str) -> Vec<HighlightToken> {
        let syntax = self.syntax_set.find_syntax_by_extension(extension).unwrap();

        // let mut h = HighlightLines::new(syntax, &self.theme_set.themes["Solarized (light)"]);
        let mut h = HighlightLines::new(syntax, &self.theme_set.themes["Solarized (dark)"]);

        let ranges: Vec<(Style, &str)> = h.highlight_line(line, &self.syntax_set).unwrap();

        // let escaped = as_24_bit_terminal_escaped(&ranges[..], false);

        // println!("\n{}", line);
        // println!("{}", escaped);

        let mut offset = 0;
        ranges
            .iter()
            .filter_map(|(style, text)| {
                let chars_count = text.chars().count();
                offset += chars_count;
                if text.trim().is_empty() {
                    None
                } else {
                    Some(HighlightToken {
                        highlight_args: HighlightArgs::from_style(style),
                        range: (offset - chars_count..offset),
                    })
                }
            })
            .collect::<Vec<_>>()
    }
}
