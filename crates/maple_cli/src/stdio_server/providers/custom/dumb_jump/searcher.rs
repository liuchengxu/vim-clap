use std::path::Path;

use anyhow::Result;
use itertools::Itertools;
use rayon::prelude::*;

use super::QueryInfo;
use crate::dumb_analyzer::{
    AddressableUsage, CtagsSearcher, GtagsSearcher, QueryType, RegexSearcher, Usage, Usages,
};
use crate::tools::ctags::{get_language, TagsConfig};
use crate::utils::ExactOrInverseTerms;

/// Context for performing a search.
#[derive(Debug, Clone, Default)]
pub(super) struct SearchingWorker {
    pub cwd: String,
    pub extension: String,
    pub query_info: QueryInfo,
}

impl SearchingWorker {
    fn ctags_search(self) -> Result<Usages> {
        let ignorecase = self.query_info.keyword.chars().all(char::is_lowercase);

        let mut tags_config = TagsConfig::with_dir(self.cwd);
        if let Some(language) = get_language(&self.extension) {
            tags_config.languages(language.into());
        }

        let QueryInfo {
            keyword,
            query_type,
            filtering_terms,
        } = self.query_info;

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

    fn gtags_search(self) -> Result<Vec<AddressableUsage>> {
        let QueryInfo {
            keyword,
            filtering_terms,
            ..
        } = self.query_info;
        let mut gtags_usages = GtagsSearcher::new(self.cwd.into())
            .search_references(&keyword)?
            .par_bridge()
            .filter_map(|tag_info| {
                let (kind, kind_weight) =
                    crate::dumb_analyzer::reference_kind(&tag_info.pattern, &self.extension);
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
            .into_par_iter()
            .map(GtagsUsage::into_addressable_usage)
            .collect::<Vec<_>>())
    }

    async fn regex_search(self) -> Result<Usages> {
        let QueryInfo {
            keyword,
            filtering_terms,
            ..
        } = self.query_info;
        let searcher = RegexSearcher {
            word: keyword,
            extension: self.extension,
            dir: Some(self.cwd.into()),
        };
        searcher.search_usages(false, &filtering_terms).await
    }
}

/// Used for sorting the usages from gtags properly.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct GtagsUsage {
    line: String,
    indices: Vec<usize>,
    line_number: usize,
    path: String,
    kind_weight: usize,
}

impl GtagsUsage {
    fn into_usage(self) -> Usage {
        let Self { line, indices, .. } = self;
        Usage { line, indices }
    }

    fn into_addressable_usage(self) -> AddressableUsage {
        AddressableUsage {
            line: self.line,
            indices: self.indices,
            path: self.path,
            line_number: self.line_number,
        }
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

/// Returns a combo of various results in the order of [ctags, gtags, regex].
fn merge_all(
    ctag_results: Usages,
    maybe_gtags_results: Option<Vec<AddressableUsage>>,
    regex_results: Usages,
) -> Usages {
    let mut regex_results = regex_results;
    regex_results.retain(|r| !ctag_results.contains(r));

    let mut ctag_results = ctag_results;
    if let Some(gtags_results) = maybe_gtags_results {
        let gtags_results = gtags_results
            .into_iter()
            .map(Into::into)
            .collect::<Vec<_>>();
        regex_results.retain(|r| !gtags_results.contains(r));
        ctag_results.append(gtags_results.into());
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
    pub async fn run(&self, searching_worker: SearchingWorker) -> Result<Usages> {
        let ctags_future = {
            let searching_worker = searching_worker.clone();
            async move { searching_worker.ctags_search() }
        };

        match self {
            SearchEngine::Ctags => searching_worker.ctags_search(),
            SearchEngine::Regex => searching_worker.regex_search().await,
            SearchEngine::CtagsAndRegex => {
                let regex_future = searching_worker.regex_search();
                let (ctags_results, regex_results) = futures::join!(ctags_future, regex_future);

                Ok(merge_all(
                    ctags_results.unwrap_or_default(),
                    None,
                    regex_results.unwrap_or_default(),
                ))
            }
            SearchEngine::CtagsElseRegex => {
                let results = searching_worker.clone().ctags_search();
                // tags might be incomplete, try the regex way if no results from the tags file.
                let try_regex =
                    results.is_err() || results.as_ref().map(|r| r.is_empty()).unwrap_or(false);
                if try_regex {
                    searching_worker.regex_search().await
                } else {
                    results
                }
            }
            SearchEngine::All => {
                let gtags_future = {
                    let searching_worker = searching_worker.clone();
                    async move { searching_worker.gtags_search() }
                };
                let regex_future = searching_worker.regex_search();

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
