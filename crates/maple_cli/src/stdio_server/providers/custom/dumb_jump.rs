use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use anyhow::{anyhow, Result};
use crossbeam_channel::Sender;
use itertools::Itertools;
use rayon::prelude::*;
use serde::Deserialize;
use serde_json::json;

use filter::Query;

use crate::dumb_analyzer::{Filtering, RegexSearcher, TagSearcher, Usage, Usages};
use crate::stdio_server::{
    providers::builtin::OnMoveHandler,
    rpc::Call,
    session::{EventHandler, NewSession, Session, SessionContext, SessionEvent},
    write_response, MethodCall,
};
use crate::tools::ctags::{get_language, TagsConfig};
use crate::utils::ExactOrInverseTerms;

/// Internal reprentation of user input.
#[derive(Debug, Clone, Default)]
struct SearchInfo {
    /// Keyword for the tag or regex searching.
    keyword: String,
    /// Search terms for further filtering.
    filtering_terms: ExactOrInverseTerms,
}

impl SearchInfo {
    /// Returns true if the filtered results of `self` is a superset of applying `other` on the same
    /// source.
    fn has_superset_results(&self, other: &Self) -> bool {
        // - the keyword is the same.
        // - the new query is a subset of last query.
        self.keyword == other.keyword && self.filtering_terms.contains(&other.filtering_terms)
    }
}

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

    let usages = TagSearcher::new(tags_config)
        .search(query, filtering, true)?
        .filter_map(|tag_line| {
            let (line, indices) = tag_line.grep_format(query, ignorecase);
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
fn combine(tag_results: Usages, regex_results: Usages) -> Usages {
    let mut regex_results = regex_results;
    regex_results.retain(|r| !tag_results.contains(r));
    let mut tag_results = tag_results;
    tag_results.append(regex_results);
    tag_results
}

/// Parses the raw user input and returns the final keyword as well as the constraint terms.
/// Currently, only one keyword is supported.
///
/// `hel 'fn` => `keyword ++ exact_term/inverse_term`.
///
/// # Argument
///
/// - `query`: Initial query typed in the input window.
fn parse_search_info(query: &str) -> SearchInfo {
    let Query {
        exact_terms,
        inverse_terms,
        fuzzy_terms,
    } = Query::from(query);

    // If there is no fuzzy term, use the full query as the keyword,
    // otherwise restore the fuzzy query as the keyword we are going to search.
    let (keyword, filtering_terms) = if fuzzy_terms.is_empty() {
        if !exact_terms.is_empty() {
            (
                exact_terms[0].word.clone(),
                ExactOrInverseTerms {
                    exact_terms,
                    inverse_terms,
                },
            )
        } else {
            (query.into(), ExactOrInverseTerms::default())
        }
    } else {
        (
            fuzzy_terms.iter().map(|term| &term.word).join(" "),
            ExactOrInverseTerms {
                exact_terms,
                inverse_terms,
            },
        )
    };

    SearchInfo {
        keyword,
        filtering_terms,
    }
}

#[derive(Debug, Clone, Default)]
struct SearchResults {
    /// Last searching results.
    ///
    /// When passing the line content from Vim to Rust, the performance
    /// of Vim can become very bad because some lines are extremely long,
    /// we cache the last results on Rust to allow passing the line number
    /// from Vim later instead.
    usages: Usages,
    /// Last raw query.
    raw_query: String,
    /// Last parsed search info.
    search_info: SearchInfo,
}

#[derive(Deserialize)]
struct Params {
    cwd: String,
    query: String,
    extension: String,
}

#[inline]
fn parse_msg(msg: MethodCall) -> (u64, Params) {
    (msg.id, msg.parse_unsafe())
}

#[allow(unused)]
enum SearchEngine {
    Ctags,
    Regex,
    Both,
}

async fn handle_dumb_jump_message(
    msg_id: u64,
    params: Params,
    search_engine: SearchEngine,
    force_execute: bool,
) -> SearchResults {
    let Params {
        cwd,
        query,
        extension,
    } = params;

    if query.is_empty() {
        return Default::default();
    }

    let SearchInfo {
        keyword,
        filtering_terms,
    } = parse_search_info(query.as_ref());

    let usages_result = match search_engine {
        SearchEngine::Ctags => {
            let results = search_tags(Path::new(&cwd), &extension, &keyword, &filtering_terms);
            // tags might be incomplete, try the regex way if no results from the tags file.
            let try_regex =
                results.is_err() || results.as_ref().map(|r| r.is_empty()).unwrap_or(false);
            if try_regex {
                search_regex(keyword.clone(), extension, cwd, &filtering_terms).await
            } else {
                results
            }
        }
        SearchEngine::Regex => {
            search_regex(keyword.clone(), extension, cwd, &filtering_terms).await
        }
        SearchEngine::Both => {
            let tags_future = {
                let cwd = cwd.clone();
                let keyword = keyword.clone();
                let extension = extension.clone();
                let filtering_terms = filtering_terms.clone();
                async move { search_tags(Path::new(&cwd), &extension, &keyword, &filtering_terms) }
            };
            let regex_future = search_regex(keyword.clone(), extension, cwd, &filtering_terms);

            let (tags_results, regex_results) =
                futures::future::join(tags_future, regex_future).await;

            Ok(combine(
                tags_results.unwrap_or_default(),
                regex_results.unwrap_or_default(),
            ))
        }
    };

    let (response, usages) = match usages_result {
        Ok(usages) => {
            let response = {
                let total = usages.len();
                // Only show the top 200 items.
                let (lines, indices): (Vec<_>, Vec<_>) = usages
                    .iter()
                    .take(200)
                    .map(|usage| (usage.line.as_str(), usage.indices.as_slice()))
                    .unzip();
                json!({
                  "id": msg_id,
                  "provider_id": "dumb_jump",
                  "force_execute": force_execute,
                  "result": { "lines": lines, "indices": indices, "total": total },
                })
            };

            (response, usages)
        }
        Err(e) => {
            tracing::error!(error = ?e, "Error at running dumb_jump");
            let response = json!({
                "id": msg_id,
                "provider_id": "dumb_jump",
                "error": { "message": e.to_string() }
            });
            (response, Default::default())
        }
    };

    write_response(response);

    SearchResults {
        usages,
        raw_query: query,
        search_info: SearchInfo {
            keyword,
            filtering_terms,
        },
    }
}

#[derive(Debug, Clone, Default)]
pub struct DumbJumpMessageHandler {
    /// Last/Latest search results.
    results: SearchResults,
    /// Whether the tags file has been (re)-created.
    tags_regenerated: Arc<AtomicBool>,
}

impl DumbJumpMessageHandler {
    // TODO: smarter strategy to regenerate the tags?
    fn regenerate_tags(&mut self, dir: &str, extension: String) {
        let mut tags_config = TagsConfig::with_dir(dir);
        if let Some(language) = get_language(&extension) {
            tags_config.languages(language.into());
        }

        let tags_searcher = TagSearcher::new(tags_config);
        match tags_searcher.generate_tags() {
            Ok(()) => {
                self.tags_regenerated.store(true, Ordering::Relaxed);
            }
            Err(e) => {
                tracing::error!(error = ?e, "Error at generating the tags file for dumb_jump");
            }
        }
    }
}

#[async_trait::async_trait]
impl EventHandler for DumbJumpMessageHandler {
    async fn handle_on_move(
        &mut self,
        msg: MethodCall,
        context: Arc<SessionContext>,
    ) -> Result<()> {
        let msg_id = msg.id;

        let lnum = msg
            .get_u64("lnum")
            .map_err(|_| anyhow!("Missing `lnum` in {:?}", msg))?;

        // lnum is 1-indexed
        if let Some(curline) = self.results.usages.get_line((lnum - 1) as usize) {
            if let Err(error) =
                OnMoveHandler::create(&msg, &context, Some(curline.into())).map(|x| x.handle())
            {
                tracing::error!(?error, "Failed to handle OnMove event");
                write_response(json!({"error": error.to_string(), "id": msg_id }));
            }
        }

        Ok(())
    }

    async fn handle_on_typed(
        &mut self,
        msg: MethodCall,
        _context: Arc<SessionContext>,
    ) -> Result<()> {
        let (msg_id, params) = parse_msg(msg);

        let search_info = parse_search_info(&params.query);

        // Try to refilter the cached results.
        if self.results.search_info.has_superset_results(&search_info) {
            tracing::debug!(
                last_query = %self.results.raw_query,
                query = %params.query,
                ?search_info,
                "============== starting refiltering",
            );
            let refiltered = self
                .results
                .usages
                .par_iter()
                .filter_map(|Usage { line, indices }| {
                    search_info
                        .filtering_terms
                        .check_jump_line((line.clone(), indices.clone()))
                })
                .collect::<Vec<_>>();
            tracing::debug!("============== ending refiltering");
            let total = refiltered.len();
            let (lines, indices): (Vec<_>, Vec<_>) = refiltered.into_iter().take(200).unzip();
            let response = json!({
              "id": msg_id,
              "provider_id": "dumb_jump",
              "force_execute": true,
              "result": { "lines": lines, "indices": indices, "total": total },
            });
            write_response(response);
            return Ok(());
        }

        let job_future = if self.tags_regenerated.load(Ordering::Relaxed) {
            handle_dumb_jump_message(msg_id, params, SearchEngine::Both, false)
        } else {
            handle_dumb_jump_message(msg_id, params, SearchEngine::Regex, false)
        };

        let results = tokio::spawn(job_future).await.unwrap_or_else(|e| {
            tracing::error!(?e, "Failed to spawn task handle_dumb_jump_message");
            Default::default()
        });
        self.results = results;
        Ok(())
    }
}

pub struct DumbJumpSession;

impl NewSession for DumbJumpSession {
    fn spawn(call: Call) -> Result<Sender<SessionEvent>> {
        let mut handler = DumbJumpMessageHandler::default();
        let (session, session_sender) = Session::new(call.clone(), handler.clone());
        session.start_event_loop();

        let (msg_id, params) = parse_msg(call.unwrap_method_call());
        tokio::task::spawn_blocking({
            let dir = params.cwd.clone();
            let extension = params.extension.clone();
            move || handler.regenerate_tags(&dir, extension)
        });
        tokio::spawn(async move {
            handle_dumb_jump_message(msg_id, params, SearchEngine::Regex, true).await;
        });

        Ok(session_sender)
    }
}
