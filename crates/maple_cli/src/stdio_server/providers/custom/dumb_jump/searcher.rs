use std::path::Path;

use anyhow::Result;
use itertools::Itertools;
use rayon::prelude::*;

use super::{QueryInfo, SearchInfo};
use crate::dumb_analyzer::{CtagsSearcher, GtagsSearcher, QueryType, RegexSearcher, Usage, Usages};
use crate::tools::ctags::{get_language, TagsConfig};
use crate::utils::ExactOrInverseTerms;

fn search_ctags(
    SearchInfo {
        cwd,
        extension,
        query_info,
    }: SearchInfo,
) -> Result<Usages> {
    let ignorecase = query_info.keyword.chars().all(char::is_lowercase);

    let mut tags_config = TagsConfig::with_dir(cwd);
    if let Some(language) = get_language(&extension) {
        tags_config.languages(language.into());
    }

    let QueryInfo {
        keyword,
        query_type,
        filtering_terms,
    } = query_info;

    let usages = CtagsSearcher::new(tags_config)
        .search(&keyword, query_type, true)?
        .sorted_by_key(|t| t.line) // Ensure the tags are sorted as the definition goes first and then the implementations.
        .par_bridge()
        .filter_map(|tag_line| {
            let (line, indices) = tag_line.grep_format_ctags(&keyword, ignorecase);
            filtering_terms
                .check_jump_line((line, indices.unwrap_or_default()))
                .map(|(line, indices)| Usage::new(line, indices))
        })
        .collect::<Vec<_>>();

    Ok(usages.into())
}

/// Used for sorting the usages from gtags properly.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct GtagsUsage {
    pub line: String,
    pub indices: Vec<usize>,
    pub line_number: usize,
    pub path: String,
    pub kind_weight: usize,
}

impl GtagsUsage {
    pub fn into_usage(self) -> Usage {
        let Self { line, indices, .. } = self;
        Usage { line, indices }
    }
}

impl PartialOrd for GtagsUsage {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some((self.kind_weight, &self.path, self.line_number).cmp(&(
            other.kind_weight,
            &other.path,
            other.line_number,
        )))
    }
}

impl Ord for GtagsUsage {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.partial_cmp(other).unwrap()
    }
}

fn search_gtags(
    SearchInfo {
        cwd,
        query_info,
        extension,
    }: SearchInfo,
) -> Result<Usages> {
    let QueryInfo {
        keyword,
        filtering_terms,
        ..
    } = query_info;
    let mut gtags_usages = GtagsSearcher::new(cwd.into())
        .search_references(&keyword)?
        .par_bridge()
        .filter_map(|tag_info| {
            // TODO: more fine-grained reference kind
            let (kind, kind_weight) = match extension.as_str() {
                "rs" if tag_info.pattern.trim_start().starts_with("use ") => ("use", 1),
                _ => ("refs", 100),
            };
            let (line, indices) = tag_info.grep_format_gtags(kind, &keyword, false);
            filtering_terms
                .check_jump_line((line, indices.unwrap_or_default()))
                .map(|(line, indices)| GtagsUsage {
                    line,
                    indices,
                    kind_weight,
                    path: tag_info.path, // TODO: perhaps path_weight? Lower the weight of path containing `test`.
                    line_number: tag_info.line,
                })
        })
        .collect::<Vec<_>>();

    gtags_usages.par_sort_unstable_by(|a, b| a.cmp(b));

    Ok(gtags_usages
        .into_iter()
        .map(GtagsUsage::into_usage)
        .collect::<Vec<_>>()
        .into())
}

async fn search_regex(
    SearchInfo {
        cwd,
        extension,
        query_info,
    }: SearchInfo,
) -> Result<Usages> {
    let QueryInfo {
        keyword,
        filtering_terms,
        ..
    } = query_info;
    let searcher = RegexSearcher {
        word: keyword,
        extension,
        dir: Some(cwd.into()),
    };
    searcher.search_usages(false, &filtering_terms).await
}

/// Returns a combo of various results in the order of [ctags, gtags, regex].
fn merge_all(
    ctag_results: Usages,
    maybe_gtags_results: Option<Usages>,
    regex_results: Usages,
) -> Usages {
    let mut regex_results = regex_results;
    regex_results.retain(|r| !ctag_results.contains(r));

    let mut ctag_results = ctag_results;
    if let Some(gtags_results) = maybe_gtags_results {
        regex_results.retain(|r| !gtags_results.contains(r));
        ctag_results.append(gtags_results);
    }

    ctag_results.append(regex_results);
    ctag_results
}

/// The initialization of Ctags for a new project is normally
/// faster than Gtags, but once Gtags has been initialized,
/// the incremental update of Gtags should be instant enough
/// and is comparable to Ctags regarding the speed.
///
/// Regex requires no initialization.
#[derive(Debug, Clone)]
pub(super) enum SearchEngine {
    Ctags,
    Regex,
    CtagsAndRegex,
    CtagsElseRegex,
    All,
}

impl SearchEngine {
    pub async fn search_usages(&self, search_info: SearchInfo) -> Result<Usages> {
        let ctags_future = {
            let search_info = search_info.clone();
            async move { search_ctags(search_info) }
        };

        match self {
            SearchEngine::Ctags => search_ctags(search_info),
            SearchEngine::Regex => search_regex(search_info).await,
            SearchEngine::CtagsAndRegex => {
                let regex_future = search_regex(search_info);
                let (ctags_results, regex_results) = futures::join!(ctags_future, regex_future);

                Ok(merge_all(
                    ctags_results.unwrap_or_default(),
                    None,
                    regex_results.unwrap_or_default(),
                ))
            }
            SearchEngine::CtagsElseRegex => {
                let results = search_ctags(search_info.clone());
                // tags might be incomplete, try the regex way if no results from the tags file.
                let try_regex =
                    results.is_err() || results.as_ref().map(|r| r.is_empty()).unwrap_or(false);
                if try_regex {
                    search_regex(search_info).await
                } else {
                    results
                }
            }
            SearchEngine::All => {
                let gtags_future = {
                    let search_info = search_info.clone();
                    async move { search_gtags(search_info) }
                };

                let regex_future = search_regex(search_info);

                let (ctags_results, gtags_results, regex_results) =
                    futures::join!(ctags_future, gtags_future, regex_future);

                Ok(merge_all(
                    ctags_results.unwrap_or_default(),
                    gtags_results.ok(),
                    regex_results.unwrap_or_default(),
                ))
            }
        }
    }
}
