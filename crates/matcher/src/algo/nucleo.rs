use nucleo_matcher::{
    pattern::{AtomKind, CaseMatching, Normalization, Pattern},
    Config, Matcher, Utf32Str,
};
use types::{MatchResult, Score};

/// Make the arguments order same to Skim's `fuzzy_indices()`.
pub fn fuzzy_indices(
    line: &str,
    query: &str,
    case_sensitive: types::CaseMatching,
) -> Option<MatchResult> {
    let mut matcher = Matcher::new(Config::DEFAULT.match_paths());
    let mut indices = Vec::new();

    let case_matching = match case_sensitive {
        types::CaseMatching::Ignore => CaseMatching::Ignore,
        types::CaseMatching::Respect => CaseMatching::Respect,
        types::CaseMatching::Smart => CaseMatching::Smart,
    };

    let mut char_buf = Vec::new();
    let haystack = Utf32Str::new(line, &mut char_buf);
    Pattern::new(query, case_matching, Normalization::Smart, AtomKind::Fuzzy)
        .indices(haystack, &mut matcher, &mut indices)
        .map(|score| {
            MatchResult::new(
                score as Score,
                indices.into_iter().map(|idx| idx as usize).collect(),
            )
        })
}
