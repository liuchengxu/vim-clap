use super::QueryInfo;
use crate::find_usages::{AddressableUsage, CtagsSearcher, GtagsSearcher, RegexSearcher, Usages};
use crate::tools::ctags::{get_language, TagsGenerator};
use maple_config::IgnoreConfig;
use paths::AbsPathBuf;
use rayon::prelude::*;
use std::collections::HashSet;
use std::io::Result;
use std::path::Path;
use std::process::{Command, Stdio};

/// `dumb_jump` search worker.
#[derive(Debug, Clone)]
pub(super) struct SearchWorker {
    pub cwd: AbsPathBuf,
    pub query_info: QueryInfo,
    pub source_file_extension: String,
}

impl SearchWorker {
    fn ctags_search(self) -> Result<Vec<AddressableUsage>> {
        let mut tags_generator = TagsGenerator::with_dir(self.cwd);
        if let Some(language) = get_language(&self.source_file_extension) {
            tags_generator.set_languages(language.into());
        }

        let QueryInfo {
            keyword,
            query_type,
            usage_matcher,
        } = self.query_info;

        CtagsSearcher::new(tags_generator).search_usages(&keyword, &usage_matcher, query_type, true)
    }

    fn gtags_search(self) -> Result<Vec<AddressableUsage>> {
        let QueryInfo {
            keyword,
            usage_matcher,
            ..
        } = self.query_info;
        GtagsSearcher::new(self.cwd.into()).search_usages(
            &keyword,
            &usage_matcher,
            &self.source_file_extension,
        )
    }

    fn regex_search(self) -> Result<Vec<AddressableUsage>> {
        let QueryInfo {
            keyword,
            usage_matcher,
            ..
        } = self.query_info;
        let regex_searcher = RegexSearcher {
            word: keyword,
            extension: self.source_file_extension,
            dir: Some(self.cwd.into()),
        };
        regex_searcher.search_usages(false, &usage_matcher)
    }
}

/// Returns a combo of various results in the order of [ctags, gtags, regex].
///
/// The regex results will be deduplicated from the results of ctags and gtags.
fn merge_all(
    ctag_results: Vec<AddressableUsage>,
    maybe_gtags_results: Option<Vec<AddressableUsage>>,
    regex_results: Vec<AddressableUsage>,
) -> Vec<AddressableUsage> {
    let mut regex_results = regex_results;
    regex_results.retain(|r| !ctag_results.contains(r));

    let mut results = ctag_results;
    if let Some(mut gtags_results) = maybe_gtags_results {
        regex_results.retain(|r| !gtags_results.contains(r));
        results.append(&mut gtags_results);
    }

    results.append(&mut regex_results);

    results
}

/// These is no best option here, each search engine has its own advantages and
/// disadvantages, hence, we make use of all of them to achieve a comprehensive
/// result.
///
/// # Comparison between all the search engines
///
/// |                | Ctags | Gtags                     | Regex                        |
/// | ----           | ----  | ----                      | ----                         |
/// | Initialization | No    | Required                  | No                           |
/// | Create         | Fast  | Slow                      | Fast                         |
/// | Update         | Fast  | Fast                      | Fast                         |
/// | Support        | Defs  | Defs(unpolished) and refs | Defs and refs(less accurate) |
///
/// The initialization of Ctags for a new project is normally
/// faster than Gtags, but once Gtags has been initialized,
/// the incremental update of Gtags should be instant enough
/// and is comparable to Ctags regarding the speed.
///
/// Regex requires no initialization.
#[derive(Debug, Clone)]
#[allow(unused)]
pub(super) enum SearchEngine {
    Ctags,
    Regex,
    CtagsAndRegex,
    CtagsElseRegex,
    All,
}

impl SearchEngine {
    pub async fn run(&self, search_worker: SearchWorker) -> Result<Usages> {
        let cwd = search_worker.cwd.clone();

        let ctags_future = {
            let search_worker = search_worker.clone();
            async move { search_worker.ctags_search() }
        };

        let regex_future = {
            let search_worker = search_worker.clone();
            async move { search_worker.regex_search() }
        };

        let addressable_usages = match self {
            SearchEngine::Ctags => search_worker.ctags_search()?,
            SearchEngine::Regex => search_worker.regex_search()?,
            SearchEngine::CtagsAndRegex => {
                let (ctags_results, regex_results) = futures::join!(ctags_future, regex_future);

                merge_all(
                    ctags_results.unwrap_or_default(),
                    None,
                    regex_results.unwrap_or_default(),
                )
            }
            SearchEngine::CtagsElseRegex => {
                let results = search_worker.clone().ctags_search();
                // tags might be incomplete, try the regex way if no results from the tags file.
                let try_regex =
                    results.is_err() || results.as_ref().map(|r| r.is_empty()).unwrap_or(false);
                if try_regex {
                    search_worker.regex_search()?
                } else {
                    results?
                }
            }
            SearchEngine::All => {
                let gtags_future = {
                    let search_worker = search_worker.clone();
                    async move { search_worker.gtags_search() }
                };

                let (ctags_results, gtags_results, regex_results) =
                    futures::join!(ctags_future, gtags_future, regex_future);

                merge_all(
                    ctags_results.unwrap_or_default(),
                    gtags_results.ok(),
                    regex_results.unwrap_or_default(),
                )
            }
        };

        let addressable_usages = filter_usages(&cwd, addressable_usages);

        Ok(addressable_usages.into())
    }
}

fn filter_usages(
    cwd: &AbsPathBuf,
    addressable_usages: Vec<AddressableUsage>,
) -> Vec<AddressableUsage> {
    let IgnoreConfig {
        git_tracked_only,
        ignore_file_path_pattern,
        ..
    } = maple_config::config()
        .ignore_config("dumb_jump", cwd)
        .clone();

    let mut addressable_usages = addressable_usages;

    if git_tracked_only && utils::is_git_repo(cwd) {
        let files = addressable_usages
            .iter()
            .map(|x| x.path.as_str())
            .collect::<HashSet<_>>();

        let git_tracked = files
            .into_par_iter()
            .filter(|path| is_git_tracked(path, cwd))
            .map(|s| s.to_string())
            .collect::<Vec<_>>();

        addressable_usages.retain(|usage| git_tracked.contains(&usage.path));
    }

    // Ignore the results from the file whose path contains `test`
    addressable_usages.retain(|usage| {
        !ignore_file_path_pattern
            .iter()
            .any(|ignore_pattern| usage.path.contains(ignore_pattern))
    });

    addressable_usages
}

fn is_git_tracked(file_path: &str, git_dir: &Path) -> bool {
    // Only the exit status matters.
    Command::new("git")
        .arg("ls-files")
        .arg("--error-unmatch")
        .arg(file_path)
        .current_dir(git_dir)
        .stderr(Stdio::null())
        .stdout(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::is_git_tracked;
    use std::time::Instant;

    #[tokio::test]
    async fn test_git2_and_git_executable() {
        let dir = std::env::current_dir().unwrap();

        let dir = dir.parent().unwrap().parent().unwrap();

        let now = Instant::now();
        let exists = is_git_tracked("./autoload/clap.vim", dir);
        println!("File exists: {exists:?}");
        let elapsed = now.elapsed();
        println!("Elapsed: {elapsed:.3?}");

        let now = Instant::now();
        let repo = git::Repository::open(dir).expect("Not a git repo");
        let elapsed = now.elapsed();
        println!("Open repository elapsed: {elapsed:.3?}");
        let now = Instant::now();
        let status = repo.status_file(std::path::Path::new("autoload/clap1.vim"));
        println!("File status: {status:?}");
        let elapsed = now.elapsed();
        println!("Elapsed: {elapsed:.3?}");
    }
}
