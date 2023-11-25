use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::str::FromStr;
use std::sync::Arc;
use tree_sitter_highlight::{Highlight, HighlightConfiguration};

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
struct HighlightConfig {
    highlight_name_and_groups: Vec<(String, String)>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
struct Config {
    language: BTreeMap<String, HighlightConfig>,
}

#[derive(Debug)]
struct HighlightConfigInner {
    highlight_names: Vec<String>,
    highlight_groups: Vec<String>,
}

#[derive(Debug)]
struct ConfigInner {
    language: BTreeMap<Language, HighlightConfigInner>,
}

static CONFIG: Lazy<ConfigInner> = Lazy::new(|| {
    let tree_sitter_config = include_bytes!("../tree_sitter_config.toml");
    let config: Config = toml::from_slice(tree_sitter_config).unwrap();
    config.into_config_inner()
});

impl Config {
    fn into_config_inner(self) -> ConfigInner {
        ConfigInner {
            language: self
                .language
                .into_iter()
                .map(|(lang, highlight_config)| {
                    let lang: Language = lang.parse().unwrap();
                    let (names, groups): (Vec<_>, Vec<_>) = highlight_config
                        .highlight_name_and_groups
                        .into_iter()
                        .unzip();
                    (
                        lang,
                        HighlightConfigInner {
                            highlight_names: names,
                            highlight_groups: groups,
                        },
                    )
                })
                .collect(),
        }
    }
}

/// Small macro to generate a module, declaring the list of highlight name
/// in tree_sitter_highlight and associated vim highlight group name.
macro_rules! highlight_names_module {
    ( $mod_name:ident; $( ($name:expr, $group:expr) ),* $(,)?) => {
        mod $mod_name {
            pub(super) const HIGHLIGHT_NAMES: &'static [&'static str] = &[
                $( $name ),*
            ];
            pub(super) const HIGHLIGHT_GROUPS: &'static [&'static str] = &[
                $( $group ),*
            ];
        }
    };
}

// Bash
highlight_names_module![
  builtin;
    ("comment", "Comment"),
    ("conditional", "Conditional"),
    ("constant", "Constant"),
    ("constant.builtin", "Constant"),
    ("function", "Function"),
    ("function.builtin", "Special"),
    ("function.macro", "Macro"),
    ("keyword", "Keyword"),
    ("label", "Label"),
    ("number", "Number"),
    ("operator", "Operator"),
    ("property", "Identifier"),
    ("punctuation.delimiter", "Delimiter"),
    ("punctuation.special", "Special"),
    ("string", "String"),
    ("string.escape", "String"),
    ("string.special", "SpecialChar"),
    ("tag", "Tag"),
    ("type", "Type"),
    ("type.definition", "Typedef"),
    ("type.builtin", "Type"),
    ("punctuation", "Delimiter"),
    ("punctuation.bracket", "Delimiter"),
    ("variable", "Identifier"),
    ("variable.builtin", "Identifier"),
    ("variable.parameter", "Identifier"),
];

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub enum Language {
    Bash,
    C,
    Cpp,
    Go,
    Javascript,
    Json,
    Markdown,
    Python,
    Rust,
    Toml,
    Viml,
}

impl FromStr for Language {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let language = match s.to_ascii_lowercase().as_str() {
            "bash" => Self::Bash,
            "c" => Self::C,
            "go" => Self::Go,
            "javascript" => Self::Javascript,
            "json" => Self::Json,
            "markdown" => Self::Markdown,
            "python" => Self::Python,
            "rust" => Self::Rust,
            "toml" => Self::Toml,
            "viml" => Self::Viml,
            _ => return Err(format!("Unknown langauge: {s}")),
        };
        Ok(language)
    }
}

impl Language {
    /// Constructs a new instance of [`Language`] from the file extension if any.
    pub fn try_from_extension(extension: &str) -> Option<Self> {
        let language = match extension {
            "sh" => Self::Bash,
            "c" | "h" => Self::C,
            "cpp" | "cxx" | "cc" | "c++" | "hpp" | "hxx" | "hh" | "h++" => Self::Cpp,
            "go" => Self::Go,
            "js" | "cjs" | "mjs" => Self::Javascript,
            "json" => Self::Json,
            "md" => Self::Markdown,
            "py" | "pyi" | "pyc" | "pyd" | "pyw" => Self::Python,
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
            "sh" => Self::Bash,
            "c" => Self::C,
            "cpp" => Self::Cpp,
            "go" => Self::Go,
            "javascript" => Self::Javascript,
            "json" => Self::Json,
            "markdown" => Self::Markdown,
            "python" => Self::Python,
            "rust" => Self::Rust,
            "toml" => Self::Toml,
            "vim" => Self::Viml,
            _ => return None,
        };

        Some(language)
    }

    pub fn highlight_name(&self, highlight: Highlight) -> &'static str {
        match &CONFIG.language.get(self) {
            Some(config) => &config.highlight_names[highlight.0],
            None => builtin::HIGHLIGHT_NAMES[highlight.0],
        }
    }

    pub fn highlight_group(&self, highlight: Highlight) -> &'static str {
        match &CONFIG.language.get(self) {
            Some(config) => &config.highlight_groups[highlight.0],
            None => builtin::HIGHLIGHT_GROUPS[highlight.0],
        }
    }

    pub fn highlight_query(&self) -> &str {
        match self {
            Self::Bash => tree_sitter_bash::HIGHLIGHT_QUERY,
            Self::C => tree_sitter_c::HIGHLIGHT_QUERY,
            Self::Cpp => tree_sitter_cpp::HIGHLIGHT_QUERY,
            Self::Go => tree_sitter_go::HIGHLIGHT_QUERY,
            Self::Javascript => tree_sitter_javascript::HIGHLIGHT_QUERY,
            Self::Json => tree_sitter_json::HIGHLIGHT_QUERY,
            Self::Markdown => tree_sitter_md::HIGHLIGHT_QUERY_BLOCK,
            Self::Python => tree_sitter_python::HIGHLIGHT_QUERY,
            Self::Rust => tree_sitter_rust::HIGHLIGHT_QUERY,
            Self::Toml => tree_sitter_toml::HIGHLIGHT_QUERY,
            Self::Viml => tree_sitter_vim::HIGHLIGHT_QUERY,
        }
    }

    fn create_new_highlight_config(&self) -> HighlightConfiguration {
        let create_config_result = match self {
            Language::Bash => HighlightConfiguration::new(
                tree_sitter_bash::language(),
                tree_sitter_bash::HIGHLIGHT_QUERY,
                "",
                "",
            ),
            Language::C => HighlightConfiguration::new(
                tree_sitter_c::language(),
                tree_sitter_c::HIGHLIGHT_QUERY,
                "",
                "",
            ),
            Language::Cpp => HighlightConfiguration::new(
                tree_sitter_cpp::language(),
                tree_sitter_cpp::HIGHLIGHT_QUERY,
                "",
                "",
            ),
            Language::Go => HighlightConfiguration::new(
                tree_sitter_go::language(),
                tree_sitter_go::HIGHLIGHT_QUERY,
                "",
                "",
            ),
            Language::Javascript => HighlightConfiguration::new(
                tree_sitter_javascript::language(),
                tree_sitter_javascript::HIGHLIGHT_QUERY,
                "",
                "",
            ),
            Language::Json => HighlightConfiguration::new(
                tree_sitter_json::language(),
                tree_sitter_json::HIGHLIGHT_QUERY,
                "",
                "",
            ),
            Language::Markdown => HighlightConfiguration::new(
                tree_sitter_md::language(),
                tree_sitter_md::HIGHLIGHT_QUERY_BLOCK,
                "",
                "",
            ),
            Language::Python => HighlightConfiguration::new(
                tree_sitter_python::language(),
                tree_sitter_python::HIGHLIGHT_QUERY,
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

        match &CONFIG.language.get(self) {
            Some(conf) => {
                config.configure(conf.highlight_names.as_slice());
            }
            None => {
                config.configure(builtin::HIGHLIGHT_NAMES);
            }
        }

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

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn test_tree_sitter_config() {
        assert_eq!(CONFIG.language.len(), 7);
    }
}
