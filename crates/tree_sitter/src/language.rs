use std::{cell::RefCell, collections::HashMap, sync::Arc};

use tree_sitter_highlight::HighlightConfiguration;

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub enum Language {
    Rust,
    Toml,
    Viml,
}

impl Language {
    /// Constructs a new instance of [`Language`] from the file extension if any.
    pub fn try_from_extension(extension: &str) -> Option<Self> {
        let language = match extension {
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
            "rust" => Self::Rust,
            "toml" => Self::Toml,
            "vim" => Self::Viml,
            _ => return None,
        };

        Some(language)
    }
}

thread_local! {
    static HIGHLIGHT_CONFIGS: RefCell<HashMap<Language, Arc<HighlightConfiguration>>> = Default::default();
}

pub fn get_highlight_config(
    language: Language,
    highlight_names: &[&str],
) -> Arc<HighlightConfiguration> {
    HIGHLIGHT_CONFIGS.with(|configs| {
        let mut configs = configs.borrow_mut();
        let config = configs
            .entry(language)
            .or_insert_with(|| Arc::new(create_new_highlight_config(language, highlight_names)));
        config.clone()
    })
}

fn create_new_highlight_config(
    language: Language,
    highlight_names: &[&str],
) -> HighlightConfiguration {
    let create_config_result = match language {
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

    config.configure(highlight_names);

    config
}
