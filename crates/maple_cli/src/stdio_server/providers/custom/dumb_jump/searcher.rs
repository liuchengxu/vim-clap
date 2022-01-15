use std::path::Path;

use anyhow::Result;

use crate::dumb_analyzer::{CtagsSearcher, Filtering, GtagsSearcher, RegexSearcher, Usage, Usages};
use crate::tools::ctags::{get_language, TagsConfig};
use crate::utils::ExactOrInverseTerms;

fn search_ctags(
    dir: &Path,
    extension: &str,
    query: &str,
    filtering_terms: &ExactOrInverseTerms,
) -> Result<Usages> {
    let ignorecase = query.chars().all(char::is_lowercase);

    let mut tags_config = TagsConfig::with_dir(dir);
    if let Some(language) = get_language(extension) {
        tags_config.languages(language.into());
    }

    let (query, filtering) = if let Some(stripped) = query.strip_suffix('*') {
        (stripped, Filtering::Contain)
    } else {
        (query, Filtering::StartWith)
    };

    let usages = CtagsSearcher::new(tags_config)
        .search(query, filtering, true)?
        .filter_map(|tag_line| {
            let (line, indices) = tag_line.grep_format_ctags(query, ignorecase);
            filtering_terms
                .check_jump_line((line, indices.unwrap_or_default()))
                .map(|(line, indices)| Usage::new(line, indices))
        })
        .collect::<Vec<_>>();

    Ok(usages.into())
}

fn search_gtags(dir: &Path, query: &str, filtering_terms: &ExactOrInverseTerms) -> Result<Usages> {
    let usages = GtagsSearcher::new(dir.to_path_buf())
        .search_references(query)?
        .filter_map(|tag_info| {
            let (line, indices) = tag_info.grep_format_gtags("refs", query, false);
            filtering_terms
                .check_jump_line((line, indices.unwrap_or_default()))
                .map(|(line, indices)| Usage::new(line, indices))
        })
        .collect::<Vec<_>>();
    Ok(usages.into())
}

async fn search_regex(
    word: String,
    extension: String,
    cwd: String,
    filtering_terms: &ExactOrInverseTerms,
) -> Result<Usages> {
    let searcher = RegexSearcher {
        word,
        extension,
        dir: Some(cwd.into()),
    };
    searcher.search_usages(false, filtering_terms).await
}

// Returns a combo of various results in the order of [ctags, gtags, regex].
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

// The initialization of Ctags is normally faster than Gtags.
// Regex requires no initialization.
#[derive(Debug, Clone)]
pub(super) enum SearchEngine {
    Ctags,
    Regex,
    CtagsAndRegex,
    CtagsElseRegex,
    All,
}

impl SearchEngine {
    pub async fn search_usages(
        &self,
        cwd: String,
        extension: String,
        keyword: String,
        filtering_terms: &ExactOrInverseTerms,
    ) -> Result<Usages> {
        let ctags_future = {
            let cwd = cwd.clone();
            let keyword = keyword.clone();
            let extension = extension.clone();
            let filtering_terms = filtering_terms.clone();
            async move { search_ctags(Path::new(&cwd), &extension, &keyword, &filtering_terms) }
        };

        match self {
            SearchEngine::Ctags => {
                search_ctags(Path::new(&cwd), &extension, &keyword, filtering_terms)
            }
            SearchEngine::Regex => search_regex(keyword, extension, cwd, filtering_terms).await,
            SearchEngine::CtagsAndRegex => {
                let regex_future = search_regex(keyword, extension, cwd, filtering_terms);
                let (ctags_results, regex_results) = futures::join!(ctags_future, regex_future);

                Ok(merge_all(
                    ctags_results.unwrap_or_default(),
                    None,
                    regex_results.unwrap_or_default(),
                ))
            }
            SearchEngine::CtagsElseRegex => {
                let results = search_ctags(Path::new(&cwd), &extension, &keyword, filtering_terms);
                // tags might be incomplete, try the regex way if no results from the tags file.
                let try_regex =
                    results.is_err() || results.as_ref().map(|r| r.is_empty()).unwrap_or(false);
                if try_regex {
                    search_regex(keyword, extension, cwd, filtering_terms).await
                } else {
                    results
                }
            }
            SearchEngine::All => {
                let gtags_future = {
                    let cwd = cwd.clone();
                    let keyword = keyword.clone();
                    let filtering_terms = filtering_terms.clone();
                    async move { search_gtags(Path::new(&cwd), &keyword, &filtering_terms) }
                };

                let regex_future = search_regex(keyword, extension, cwd, filtering_terms);

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
