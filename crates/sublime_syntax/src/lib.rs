use colors_transform::{AlphaColor, Color as ColorT, Rgb};
use rgb2ansi256::rgb_to_ansi256;
use std::ops::Range;
use syntect::highlighting::{
    Color, FontStyle, HighlightIterator, HighlightState, Highlighter, Style, Theme, ThemeSet,
};
use syntect::parsing::{ParseState, ScopeStack, SyntaxSet};

pub use syntect::parsing::SyntaxReference;

pub const DEFAULT_SYNTAXSET: &[u8] = include_bytes!("../../../assets/syntaxes.bin");
pub const DEFAULT_THEMESET: &[u8] = include_bytes!("../../../assets/themes.bin");

#[derive(Debug)]
pub enum Error {
    DefaultThemeNotFound(&'static str),
    Syntect(syntect::Error),
}

/// `:h attr-list`
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub enum AttrList {
    Bold,
    Underline,
    Italic,
    #[default]
    None,
}

/// Highlight arguments to be easily consumed by Vim.
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
    fn from_style(style: Style) -> Self {
        let cterm = match style.font_style {
            FontStyle::BOLD => AttrList::Bold,
            FontStyle::UNDERLINE => AttrList::Underline,
            FontStyle::ITALIC => AttrList::Italic,
            _ => AttrList::None,
        };
        let guifg = Rgb::from_tuple(&(
            style.foreground.r as f32,
            style.foreground.g as f32,
            style.foreground.b as f32,
        ))
        .set_alpha(style.foreground.a as f32);

        let ctermfg = rgb_to_ansi256(style.foreground.r, style.foreground.g, style.foreground.b);

        let gui = cterm.clone();
        let guibg = Rgb::from_tuple(&(
            style.background.r as f32,
            style.background.g as f32,
            style.background.b as f32,
        ))
        .set_alpha(style.background.a as f32);
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

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TokenHighlight {
    pub cterm: AttrList,
    pub ctermfg: u8,
    pub ctermbg: u8,
    pub gui: AttrList,
    pub guifg: String,
    pub guibg: String,
    pub group_name: String,
    /// Token range in bytes.
    ///
    /// Start of (byte-indexed) column range to highlight.
    pub col_start: usize,
    /// Length of bytes to highlight.
    pub length: usize,
}

// TODO: patch upstream to provide a API for this purpose?
/// Replicate [`syntect::HighlightLines`] in order to reduce one allocation in
/// [`Self::highlight_line`].
struct HighlightEngine<'a> {
    highlighter: Highlighter<'a>,
    parse_state: ParseState,
    highlight_state: HighlightState,
}

impl<'a> HighlightEngine<'a> {
    fn new(syntax: &SyntaxReference, theme: &'a Theme) -> Self {
        let highlighter = Highlighter::new(theme);
        let highlight_state = HighlightState::new(&highlighter, ScopeStack::new());
        Self {
            highlighter,
            parse_state: ParseState::new(syntax),
            highlight_state,
        }
    }

    /// Returns the token highlights for this line on success.
    fn highlight_line(
        &mut self,
        line: &str,
        syntax_set: &SyntaxSet,
        maybe_normal_foreground: Option<Color>,
    ) -> Result<Vec<TokenHighlight>, syntect::Error> {
        let ops = self.parse_state.parse_line(line, syntax_set)?;

        let mut offset = 0;
        let token_highlights =
            HighlightIterator::new(&mut self.highlight_state, &ops[..], line, &self.highlighter)
                .filter_map(|(style, text)| {
                    let chars_count = text.chars().count();
                    offset += chars_count;

                    // A lot of tokens use the Normal highlight, which is done by vim syntax highlight itself.
                    let is_normal = maybe_normal_foreground
                        .map(|fg| fg == style.foreground)
                        .unwrap_or(false);

                    if text.trim().is_empty() || is_normal {
                        None
                    } else {
                        let char_indices = Vec::from_iter(offset - chars_count..offset);
                        let byte_indices = utils::char_indices_to_byte_indices(line, &char_indices);
                        let highlight_args = HighlightArgs::from_style(style);
                        let hex_guifg = highlight_args.guifg.to_css_hex_string();
                        let hex_guibg = highlight_args.guibg.to_css_hex_string();
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
                            col_start: byte_indices[0],
                            length: byte_indices.len(),
                        })
                    }
                })
                .collect::<Vec<_>>();

        Ok(token_highlights)
    }
}

#[derive(Debug)]
pub struct SyntaxHighlighter {
    pub syntax_set: SyntaxSet,
    pub theme_set: ThemeSet,
}

impl Default for SyntaxHighlighter {
    fn default() -> Self {
        Self {
            syntax_set: SyntaxSet::load_defaults_newlines(),
            theme_set: ThemeSet::load_defaults(),
        }
    }
}

impl SyntaxHighlighter {
    const DEFAULT_THEME: &'static str = "Solarized (dark)";

    /// Constructs a new instance of [`SyntaxHighlighter`].
    ///
    /// Should be called only once at the start of program.
    pub fn new() -> Self {
        Self {
            syntax_set: syntect::dumps::from_binary(DEFAULT_SYNTAXSET),
            theme_set: syntect::dumps::from_binary(DEFAULT_THEMESET),
        }
    }

    pub fn get_theme_list(&self) -> Vec<String> {
        self.theme_set.themes.keys().cloned().collect()
    }

    pub fn theme_exists(&self, theme: &str) -> bool {
        self.theme_set.themes.contains_key(theme)
    }

    /// Converts the foreground color of the theme to Normal highlight
    pub fn get_normal_highlight(&self, theme: &str) -> Option<(String, u8)> {
        if let Some(normal_fg_color) = self
            .theme_set
            .themes
            .get(theme)
            .and_then(|theme| theme.settings.foreground)
        {
            let guifg = Rgb::from_tuple(&(
                normal_fg_color.r as f32,
                normal_fg_color.g as f32,
                normal_fg_color.b as f32,
            ))
            .set_alpha(normal_fg_color.a as f32);

            let ctermfg = rgb_to_ansi256(normal_fg_color.r, normal_fg_color.g, normal_fg_color.b);

            Some((guifg.to_css_hex_string(), ctermfg))
        } else {
            None
        }
    }

    pub fn get_token_highlights_in_line(
        &self,
        syntax: &SyntaxReference,
        line: &str,
        theme: &str,
    ) -> Result<Vec<TokenHighlight>, Error> {
        let theme = match self.theme_set.themes.get(theme) {
            Some(v) => v,
            None => self
                .theme_set
                .themes
                .get(Self::DEFAULT_THEME)
                .ok_or(Error::DefaultThemeNotFound(Self::DEFAULT_THEME))?,
        };
        HighlightEngine::new(syntax, theme)
            .highlight_line(line, &self.syntax_set, theme.settings.foreground)
            .map_err(Error::Syntect)
    }

    pub fn highlight_line(&self, extension: &str, line: &str) -> Vec<TokenHighlighterForTerminal> {
        let syntax = self.syntax_set.find_syntax_by_extension(extension).unwrap();

        let mut h =
            syntect::easy::HighlightLines::new(syntax, &self.theme_set.themes["Solarized (dark)"]);

        let ranges: Vec<(Style, &str)> = h
            .highlight_line(line, &self.syntax_set)
            .expect("Failed to parse line");

        let mut offset = 0;
        ranges
            .into_iter()
            .filter_map(|(style, text)| {
                let chars_count = text.chars().count();
                offset += chars_count;
                if text.trim().is_empty() {
                    None
                } else {
                    Some(TokenHighlighterForTerminal {
                        highlight_args: HighlightArgs::from_style(style),
                        range: (offset - chars_count..offset),
                    })
                }
            })
            .collect::<Vec<_>>()
    }
}

#[derive(Debug)]
pub struct TokenHighlighterForTerminal {
    pub highlight_args: HighlightArgs,
    /// Token range in chars.
    pub range: Range<usize>,
}
