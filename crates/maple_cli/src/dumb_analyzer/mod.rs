mod search_based;

pub use self::search_based::{
    definitions_and_references, definitions_and_references_lines, find_occurrence_matches_by_ext,
    get_comments_by_ext, get_language_by_ext, DefinitionRules, MatchKind,
};
