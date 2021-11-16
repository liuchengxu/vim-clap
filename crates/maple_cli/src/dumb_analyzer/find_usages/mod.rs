//! This module provides the feature of search based `jump-to-definition`, inspired
//! by https://github.com/jacktasia/dumb-jump, powered by a set of regular expressions
//! based on the file extension, using the ripgrep tool.
//!
//! The matches are run through a shared set of heuristic methods to find the best candidate.
//!
//! # Dependency
//!
//! The executable rg with `--json` and `--pcre2` is required to be installed on the system.

mod default_types;
mod definition;
mod search;

pub use self::definition::{
    definitions_and_references, definitions_and_references_lines, get_comments_by_ext,
    get_language_by_ext, DefinitionRules, MatchKind,
};
pub use self::search::{
    find_definition_matches_with_kind, find_occurrence_matches_by_ext, find_occurrences_by_lang,
    naive_grep_fallback,
};
