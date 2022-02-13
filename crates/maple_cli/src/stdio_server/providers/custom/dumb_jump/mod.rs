mod searcher;

use std::ops::Deref;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use anyhow::{anyhow, Result};
use itertools::Itertools;
use rayon::prelude::*;
use serde::Deserialize;
use serde_json::json;

use filter::Query;

use self::searcher::SearchEngine;
use crate::dumb_analyzer::{CtagsSearcher, GtagsSearcher, SearchType, Usage, Usages};
use crate::stdio_server::{
    providers::builtin::OnMoveHandler,
    rpc::Call,
    session::{note_job_is_finished, register_job_successfully, EventHandle, SessionContext},
    write_response, MethodCall,
};
use crate::tools::ctags::{get_language, TagsConfig};
use crate::utils::ExactOrInverseTerms;

/// Internal reprentation of user input.
#[derive(Debug, Clone, Default)]
struct SearchInfo {
    /// Keyword for the tag or regex searching.
    keyword: String,
    /// Search type for `keyword`.
    search_type: SearchType,
    /// Search terms for further filtering.
    filtering_terms: ExactOrInverseTerms,
}

impl SearchInfo {
    /// Returns true if the filtered results of `self` is a superset of applying `other` on the
    /// same source.
    ///
    /// The rule is as follows:
    ///
    /// - the keyword is the same.
    /// - the new query is a subset of last query.
    fn has_superset_results(&self, other: &Self) -> bool {
        self.keyword == other.keyword
            && self.search_type == other.search_type
            && self.filtering_terms.contains(&other.filtering_terms)
    }
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
    let (keyword, search_type, filtering_terms) = if fuzzy_terms.is_empty() {
        if exact_terms.is_empty() {
            (
                query.into(),
                SearchType::StartWith,
                ExactOrInverseTerms::default(),
            )
        } else {
            (
                exact_terms[0].word.clone(),
                SearchType::Exact,
                ExactOrInverseTerms {
                    exact_terms,
                    inverse_terms,
                },
            )
        }
    } else {
        (
            fuzzy_terms.iter().map(|term| &term.word).join(" "),
            SearchType::StartWith,
            ExactOrInverseTerms {
                exact_terms,
                inverse_terms,
            },
        )
    };

    // TODO: Search syntax:
    // - 'foo
    // - foo*
    // - foo
    //
    // if let Some(stripped) = query.strip_suffix('*') {
    // (stripped, SearchType::Contain)
    // } else if let Some(stripped) = query.strip_prefix('\'') {
    // (stripped, SearchType::Exact)
    // } else {
    // (query, SearchType::StartWith)
    // };

    SearchInfo {
        keyword,
        search_type,
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

async fn search_for_usages(
    msg_id: u64,
    params: Params,
    maybe_search_info: Option<SearchInfo>,
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

    let search_info = maybe_search_info.unwrap_or_else(|| parse_search_info(query.as_ref()));

    let (response, usages) = match search_engine
        .search_usages(cwd, extension, &search_info)
        .await
    {
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
        search_info,
    }
}

#[derive(Debug, Clone, Default)]
pub struct DumbJumpHandle {
    /// Results from last searching.
    /// This might be a superset of searching results for the last query.
    cached_results: SearchResults,
    /// Current results from refiltering on `cached_results`.
    current_usages: Option<Usages>,
    /// Whether the tags file has been (re)-created.
    ctags_regenerated: Arc<AtomicBool>,
    /// Whether the GTAGS file has been (re)-created.
    gtags_regenerated: Arc<AtomicBool>,
}

impl DumbJumpHandle {
    /// Starts a new searching task.
    async fn start_search(
        &self,
        msg_id: u64,
        params: Params,
        search_info: SearchInfo,
    ) -> SearchResults {
        let search_engine = match (
            self.ctags_regenerated.load(Ordering::Relaxed),
            self.gtags_regenerated.load(Ordering::Relaxed),
        ) {
            (true, true) => SearchEngine::All,
            (true, false) => SearchEngine::CtagsAndRegex,
            _ => SearchEngine::Regex,
        };
        let job_future = search_for_usages(msg_id, params, Some(search_info), search_engine, false);

        tokio::spawn(job_future).await.unwrap_or_else(|e| {
            tracing::error!(?e, "Failed to spawn task search_for_usages");
            Default::default()
        })
    }
}

#[async_trait::async_trait]
impl EventHandle for DumbJumpHandle {
    async fn on_create(&mut self, call: Call, _context: Arc<SessionContext>) {
        let (msg_id, params) = parse_msg(call.unwrap_method_call());

        let job_id = utility::calculate_hash(&(&params.cwd, "dumb_jump"));

        if register_job_successfully(job_id) {
            let ctags_future = {
                let ctags_regenerated = self.ctags_regenerated.clone();
                let cwd = params.cwd.clone();
                let mut tags_config = TagsConfig::with_dir(cwd.clone());
                if let Some(language) = get_language(&params.extension) {
                    tags_config.languages(language.into());
                }

                // TODO: smarter strategy to regenerate the tags?
                async move {
                    let now = std::time::Instant::now();
                    let ctags_searcher = CtagsSearcher::new(tags_config);
                    match ctags_searcher.generate_tags() {
                        Ok(()) => {
                            ctags_regenerated.store(true, Ordering::Relaxed);
                        }
                        Err(e) => {
                            tracing::error!(error = ?e, "💔 Error at generating the tags file for dumb_jump");
                        }
                    }
                    tracing::debug!(?cwd, "⏱️  Ctags elapsed: {:?}", now.elapsed());
                }
            };

            let gtags_future = {
                let gtags_regenerated = self.gtags_regenerated.clone();
                let cwd = params.cwd.clone();
                async move {
                    let now = std::time::Instant::now();
                    let gtags_searcher = GtagsSearcher::new(cwd.clone().into());
                    match tokio::task::spawn_blocking({
                        let gtags_searcher = gtags_searcher.clone();
                        move || gtags_searcher.create_or_update_tags()
                    })
                    .await
                    {
                        Ok(_) => gtags_regenerated.store(true, Ordering::Relaxed),
                        Err(e) => {
                            tracing::error!(error = ?e, "💔 Error at initializing GTAGS, attempting to recreate...");
                            // TODO: creating gtags may take 20s+ for large project
                            match tokio::task::spawn_blocking({
                                let gtags_searcher = gtags_searcher.clone();
                                move || gtags_searcher.force_recreate()
                            })
                            .await
                            {
                                Ok(_) => {
                                    gtags_regenerated.store(true, Ordering::Relaxed);
                                    tracing::debug!("Recreating gtags db successfully");
                                }
                                Err(e) => {
                                    tracing::error!(error = ?e, "💔 Failed to recreate gtags db");
                                }
                            }
                        }
                    }
                    tracing::debug!(?cwd, "⏱️  Gtags elapsed: {:?}", now.elapsed());
                }
            };

            if *crate::tools::gtags::GTAGS_EXISTS.deref() {
                tokio::task::spawn({
                    async move {
                        let now = std::time::Instant::now();
                        futures::future::join(ctags_future, gtags_future).await;
                        tracing::debug!("⏱️  Total elapsed: {:?}", now.elapsed());
                        note_job_is_finished(job_id);
                    }
                });
            } else {
                tokio::task::spawn({
                    async move {
                        let now = std::time::Instant::now();
                        ctags_future.await;
                        tracing::debug!("⏱️  Total elapsed: {:?}", now.elapsed());
                        note_job_is_finished(job_id);
                    }
                });
            }
        }

        tokio::spawn(async move {
            search_for_usages(msg_id, params, None, SearchEngine::Regex, true).await;
        });
    }

    async fn on_move(&mut self, msg: MethodCall, context: Arc<SessionContext>) -> Result<()> {
        let msg_id = msg.id;

        let lnum = msg
            .get_u64("lnum")
            .map_err(|_| anyhow!("Missing `lnum` in {:?}", msg))?;

        // lnum is 1-indexed
        if let Some(curline) = self
            .current_usages
            .as_ref()
            .unwrap_or(&self.cached_results.usages)
            .get_line((lnum - 1) as usize)
        {
            if let Err(error) =
                OnMoveHandler::create(&msg, &context, Some(curline.into())).map(|x| x.handle())
            {
                tracing::error!(?error, "Failed to handle OnMove event");
                write_response(json!({"error": error.to_string(), "id": msg_id }));
            }
        }

        Ok(())
    }

    async fn on_typed(&mut self, msg: MethodCall, _context: Arc<SessionContext>) -> Result<()> {
        let (msg_id, params) = parse_msg(msg);

        let search_info = parse_search_info(&params.query);

        // Try to refilter the cached results.
        if self
            .cached_results
            .search_info
            .has_superset_results(&search_info)
        {
            let refiltered = self
                .cached_results
                .usages
                .par_iter()
                .filter_map(|Usage { line, indices }| {
                    search_info
                        .filtering_terms
                        .check_jump_line((line.clone(), indices.clone()))
                        .map(|(line, indices)| Usage::new(line, indices))
                })
                .collect::<Vec<_>>();
            let total = refiltered.len();
            let (lines, indices): (Vec<&str>, Vec<&[usize]>) = refiltered
                .iter()
                .take(200)
                .map(|Usage { line, indices }| (line.as_str(), indices.as_slice()))
                .unzip();
            let response = json!({
                "id": msg_id,
                "provider_id": "dumb_jump",
                "force_execute": true,
                "result": { "lines": lines, "indices": indices, "total": total },
            });
            write_response(response);
            self.current_usages.replace(refiltered.into());
            return Ok(());
        }

        self.cached_results = self.start_search(msg_id, params, search_info).await;
        self.current_usages.take();

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_search_info() {
        let search_info = parse_search_info("'foo");
        println!("{:?}", search_info);
    }
}
