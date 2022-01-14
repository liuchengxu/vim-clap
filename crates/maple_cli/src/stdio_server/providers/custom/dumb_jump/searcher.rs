use std::path::Path;

use anyhow::Result;

use crate::dumb_analyzer::{Filtering, RegexSearcher, CtagsSearcher, Usage, Usages};
use crate::tools::ctags::{get_language, TagsConfig};
use crate::utils::ExactOrInverseTerms;

fn search_tags(
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

// Returns a combo of tag results and regex results, tag results should be displayed first.
fn merge(tag_results: Usages, regex_results: Usages) -> Usages {
    let mut regex_results = regex_results;
    regex_results.retain(|r| !tag_results.contains(r));
    let mut tag_results = tag_results;
    tag_results.append(regex_results);
    tag_results
}

#[derive(Debug, Clone)]
pub(super) enum SearchEngine {
    Ctags,
    Regex,
    CtagsElseRegex,
    Both,
}

impl SearchEngine {
    pub async fn search_usages(
        &self,
        cwd: String,
        extension: String,
        keyword: String,
        filtering_terms: &ExactOrInverseTerms,
    ) -> Result<Usages> {
        match self {
            SearchEngine::Ctags => {
                search_tags(Path::new(&cwd), &extension, &keyword, filtering_terms)
            }
            SearchEngine::Regex => search_regex(keyword, extension, cwd, filtering_terms).await,
            SearchEngine::CtagsElseRegex => {
                let results = search_tags(Path::new(&cwd), &extension, &keyword, filtering_terms);
                // tags might be incomplete, try the regex way if no results from the tags file.
                let try_regex =
                    results.is_err() || results.as_ref().map(|r| r.is_empty()).unwrap_or(false);
                if try_regex {
                    search_regex(keyword, extension, cwd, filtering_terms).await
                } else {
                    results
                }
            }
            SearchEngine::Both => {
                let tags_future = {
                    let cwd = cwd.clone();
                    let keyword = keyword.clone();
                    let extension = extension.clone();
                    let filtering_terms = filtering_terms.clone();
                    async move { search_tags(Path::new(&cwd), &extension, &keyword, &filtering_terms) }
                };
                let regex_future = search_regex(keyword, extension, cwd, filtering_terms);

                let (tags_results, regex_results) =
                    futures::future::join(tags_future, regex_future).await;

                Ok(merge(
                    tags_results.unwrap_or_default(),
                    regex_results.unwrap_or_default(),
                ))
            }
        }
    }
}
