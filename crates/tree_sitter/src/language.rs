use std::{cell::RefCell, collections::HashMap, sync::Arc};
use tree_sitter_highlight::{Highlight, HighlightConfiguration};

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub enum Language {
    Go,
    Markdown,
    Rust,
    Toml,
    Viml,
}

/// Small macro to declare the list of highlight name in tree_sitter_highlight
/// and associated vim highlight group name.
macro_rules! highlight_names {
    ($( ($name:expr, $group:expr) ),* $(,)?) => {
        const HIGHLIGHT_NAMES: &'static [&'static str] = &[
            $( $name ),*
        ];
        const HIGHLIGHT_GROUPS: &'static [(&'static str, &'static str)] = &[
            $( ($name, $group) ),*
        ];
    };
}

impl Language {
    highlight_names![
        ("comment", "Comment"),
        ("constant", "Constant"),
        ("constant.builtin", "Constant"),
        ("function", "Function"),
        ("function.builtin", "Special"),
        ("function.macro", "Macro"),
        ("keyword", "Keyword"),
        ("operator", "Operator"),
        ("property", "Identifier"),
        ("punctuation.delimiter", "Delimiter"),
        ("punctuation.special", "Special"),
        ("string", "String"),
        ("string.escape", "String"),
        ("string.special", "SpecialChar"),
        ("type", "Type"),
        ("type.definition", "Typedef"),
        ("type.builtin", "Type"),
        ("text.literal", "SpecialChar"),
        ("text.reference", "Float"),
        ("text.title", "Title"),
        ("text.uri", "Directory"),
        ("tag", "Tag"),
        ("attribute", "Conditional"),
        ("conditional", "Conditional"),
        ("punctuation", "Delimiter"),
        ("punctuation.bracket", "Delimiter"),
        ("variable", "Identifier"),
        ("variable.builtin", "Identifier"),
        ("variable.parameter", "Identifier"),
    ];

    /// Constructs a new instance of [`Language`] from the file extension if any.
    pub fn try_from_extension(extension: &str) -> Option<Self> {
        let language = match extension {
            "go" => Self::Go,
            "md" => Self::Markdown,
            "rs" => Self::Rust,
            "toml" => Self::Toml,
            "vim" => Self::Viml,
            _ => return None,
        };

        Some(language)
    }

    /// Constructs a new instance of [`Language`] from the filetype if any.
    pub fn try_from_filetype(filetype: &str) -> Option<Self> {
        let language = match filetype {
            "go" => Self::Go,
            "markdown" => Self::Markdown,
            "rust" => Self::Rust,
            "toml" => Self::Toml,
            "vim" => Self::Viml,
            _ => return None,
        };

        Some(language)
    }

    pub fn highlight_names(&self) -> &[&str] {
        match self {
            Self::Markdown => {
                todo!()
            }
            _ => Self::HIGHLIGHT_NAMES,
        }
    }

    pub fn highlight_name(&self, highlight: Highlight) -> &'static str {
        Self::HIGHLIGHT_NAMES[highlight.0]
    }

    pub fn highlight_group(&self, highlight: Highlight) -> &'static str {
        Self::HIGHLIGHT_GROUPS[highlight.0].1
    }

    pub fn highlight_query(&self) -> &str {
        match self {
            Self::Go => tree_sitter_go::HIGHLIGHT_QUERY,
            Self::Markdown => tree_sitter_md::HIGHLIGHT_QUERY_BLOCK,
            Self::Rust => tree_sitter_rust::HIGHLIGHT_QUERY,
            Self::Toml => tree_sitter_toml::HIGHLIGHT_QUERY,
            Self::Viml => tree_sitter_vim::HIGHLIGHT_QUERY,
        }
    }

    fn create_new_highlight_config(&self) -> HighlightConfiguration {
        let create_config_result = match self {
            Language::Go => HighlightConfiguration::new(
                tree_sitter_go::language(),
                tree_sitter_go::HIGHLIGHT_QUERY,
                "",
                "",
            ),
            Language::Markdown => HighlightConfiguration::new(
                tree_sitter_md::language(),
                tree_sitter_md::HIGHLIGHT_QUERY_BLOCK,
                "",
                "",
            ),
            Language::Rust => HighlightConfiguration::new(
                tree_sitter_rust::language(),
                tree_sitter_rust::HIGHLIGHT_QUERY,
                "",
                "",
            ),
            Language::Toml => HighlightConfiguration::new(
                tree_sitter_toml::language(),
                tree_sitter_toml::HIGHLIGHT_QUERY,
                "",
                "",
            ),
            Language::Viml => HighlightConfiguration::new(
                tree_sitter_vim::language(),
                tree_sitter_vim::HIGHLIGHT_QUERY,
                "",
                "",
            ),
        };

        let mut config = create_config_result.expect("Query creation must be succeed");

        config.configure(self.highlight_names());

        config
    }
}

thread_local! {
    static HIGHLIGHT_CONFIGS: RefCell<HashMap<Language, Arc<HighlightConfiguration>>> = Default::default();
}

pub fn get_highlight_config(language: Language) -> Arc<HighlightConfiguration> {
    HIGHLIGHT_CONFIGS.with(|configs| {
        let mut configs = configs.borrow_mut();
        let config = configs
            .entry(language)
            .or_insert_with(|| Arc::new(language.create_new_highlight_config()));
        config.clone()
    })
}
