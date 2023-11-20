use std::{cell::RefCell, collections::HashMap, sync::Arc};
use tree_sitter_highlight::{Highlight, HighlightConfiguration};

/// Small macro to generate a module, declaring the list of highlight name
/// in tree_sitter_highlight and associated vim highlight group name.
macro_rules! highlight_names_module {
    ( $mod_name:ident; $( ($name:expr, $group:expr) ),* $(,)?) => {
        mod $mod_name {
            pub(super) const HIGHLIGHT_NAMES: &'static [&'static str] = &[
                $( $name ),*
            ];
            pub(super) const HIGHLIGHT_GROUPS: &'static [(&'static str, &'static str)] = &[
                $( ($name, $group) ),*
            ];
        }
    };
}

highlight_names_module! {
  c;
  ("comment", "Comment"),
  ("constant", "Constant"),
  ("delimiter", "Delimiter"),
  ("function", "Function"),
  ("function.special", "Special"),
  ("keyword", "Keyword"),
  ("label", "Label"),
  ("number", "Number"),
  ("operator", "Operator"),
  ("property", "SpecialKey"),
  ("string", "String"),
  ("type", "Type"),
  ("variable", "Identifier"),
}

highlight_names_module! {
  go;
  ("comment", "Comment"),
  ("constant.builtin", "Constant"),
  ("escape", "Delimiter"),
  ("function", "Function"),
  ("function.builtin", "Special"),
  ("function.method", "Include"),
  ("keyword", "Keyword"),
  ("number", "Number"),
  ("operator", "Operator"),
  ("property", "SpecialKey"),
  ("string", "String"),
  ("type", "Type"),
  ("variable", "Identifier"),
}

highlight_names_module! {
  markdown;
  ("none", "Normal"),
  ("punctuation.delimiter", "Delimiter"),
  ("punctuation.special", "Special"),
  ("string.escape", "String"),
  ("text.literal", "SpecialChar"),
  ("text.reference", "Float"),
  ("text.title", "Title"),
  ("text.uri", "Directory"),
}

highlight_names_module! {
  rust;
  ("attribute", "Special"),
  ("comment", "Comment"),
  ("constant", "Constant"),
  ("constant.builtin", "Constant"),
  ("constructor", "Tag"),
  ("escape", "Todo"),
  ("function", "Function"),
  ("function.macro", "Macro"),
  ("function.method", "SpecialKey"),
  ("keyword", "Keyword"),
  ("label", "Label"),
  ("operator", "Operator"),
  ("property", "Number"),
  ("punctuation.bracket", "Delimiter"),
  ("punctuation.delimiter", "Delimiter"),
  ("string", "String"),
  ("type", "Type"),
  ("type.builtin", "Type"),
  ("variable.builtin", "Identifier"),
  ("variable.parameter", "Identifier"),
}

highlight_names_module! {
  viml;
  ("_option", "StorageClass"),
  ("_scope", "Special"),
  ("boolean", "Boolean"),
  ("comment", "Comment"),
  ("conditional", "Conditional"),
  ("conditional.ternary", "Conditional"),
  ("constant", "Constant"),
  ("constant.builtin", "Constant"),
  ("exception", "Exception"),
  ("float", "Float"),
  ("function", "Function"),
  ("function.call", "Function"),
  ("function.macro", "Macro"),
  ("keyword", "Keyword"),
  ("keyword.function", "Keyword"),
  ("keyword.operator", "Keyword"),
  ("label", "Label"),
  ("namespace", "PreProc"),
  ("number", "Number"),
  ("operator", "Operator"),
  ("parameter", "Special"),
  ("property", "Identifier"),
  ("punctuation.bracket", "Delimiter"),
  ("punctuation.delimiter", "Delimiter"),
  ("punctuation.special", "Delimiter"),
  ("repeat", "Repeat"),
  ("spell", "SpellLocal"),
  ("string", "String"),
  ("string.regex", "Typedef"),
  ("string.special", "SpecialChar"),
  ("type", "Type"),
  ("variable", "Identifier"),
  ("variable.builtin", "Identifier"),
}

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

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
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

    pub fn highlight_names(&self) -> &[&str] {
        match self {
            Self::C => c::HIGHLIGHT_NAMES,
            Self::Go => go::HIGHLIGHT_NAMES,
            Self::Markdown => markdown::HIGHLIGHT_NAMES,
            Self::Rust => rust::HIGHLIGHT_NAMES,
            Self::Viml => viml::HIGHLIGHT_NAMES,
            _ => builtin::HIGHLIGHT_NAMES,
        }
    }

    pub fn highlight_name(&self, highlight: Highlight) -> &'static str {
        match self {
            Self::C => c::HIGHLIGHT_NAMES[highlight.0],
            Self::Go => go::HIGHLIGHT_NAMES[highlight.0],
            Self::Markdown => markdown::HIGHLIGHT_NAMES[highlight.0],
            Self::Rust => rust::HIGHLIGHT_NAMES[highlight.0],
            Self::Viml => viml::HIGHLIGHT_NAMES[highlight.0],
            _ => builtin::HIGHLIGHT_NAMES[highlight.0],
        }
    }

    pub fn highlight_group(&self, highlight: Highlight) -> &'static str {
        match self {
            Self::C => c::HIGHLIGHT_GROUPS[highlight.0].1,
            Self::Go => go::HIGHLIGHT_GROUPS[highlight.0].1,
            Self::Markdown => markdown::HIGHLIGHT_GROUPS[highlight.0].1,
            Self::Rust => rust::HIGHLIGHT_GROUPS[highlight.0].1,
            Self::Viml => viml::HIGHLIGHT_GROUPS[highlight.0].1,
            _ => builtin::HIGHLIGHT_GROUPS[highlight.0].1,
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
