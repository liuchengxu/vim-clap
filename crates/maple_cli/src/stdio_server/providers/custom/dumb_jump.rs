use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use anyhow::Result;
use crossbeam_channel::Sender;
use itertools::Itertools;
use serde::Deserialize;
use serde_json::json;

use filter::Query;

use crate::dumb_analyzer::{RegexSearcher, TagsSearcher, Usage, Usages};
use crate::stdio_server::{
    providers::builtin::OnMoveHandler,
    rpc::Call,
    session::{EventHandler, NewSession, Session, SessionContext, SessionEvent},
    write_response, MethodCall,
};
use crate::tools::ctags::TagsConfig;
use crate::utils::ExactOrInverseTerms;

fn search_tags(
    dir: &Path,
    query: &str,
    exact_or_inverse_terms: &ExactOrInverseTerms,
) -> Result<Usages> {
    let ignorecase = query.chars().all(char::is_lowercase);

    let usages = TagsSearcher::new(TagsConfig::with_dir(dir))
        .search(query, true)?
        .filter_map(|tag_line| {
            let (line, indices) = tag_line.grep_format(query, ignorecase);
            exact_or_inverse_terms
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
    exact_or_inverse_terms: &ExactOrInverseTerms,
) -> Result<Usages> {
    let searcher = RegexSearcher {
        word,
        extension,
        dir: Some(cwd.into()),
    };
    searcher.search_usages(false, exact_or_inverse_terms).await
}

/// When we invokes the dumb_jump provider, the search query should be `identifier(s) ++ exact_term/inverse_term`.
fn parse_raw_query(query: &str) -> (String, ExactOrInverseTerms) {
    let Query {
        exact_terms,
        inverse_terms,
        fuzzy_terms,
    } = Query::from(query);

    // If there is no fuzzy term, use the full query as the identifier,
    // otherwise restore the fuzzy query as the identifier we are going to search.
    let (identifier, exact_or_inverse_terms) = if fuzzy_terms.is_empty() {
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

    (identifier, exact_or_inverse_terms)
}

#[derive(Debug, Clone, Default)]
pub struct SearchResults {
    /// When passing the line content from Vim to Rust, for
    /// these lines that are extremely long, the performance
    /// of Vim can become very bad, we cache the display lines
    /// on Rust to pass the line number instead.
    pub lines: Vec<String>,
    /// Last query.
    pub query: String,
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

enum SearchEngine {
    Ctags,
    Regex,
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

    let (identifier, exact_or_inverse_terms) = parse_raw_query(query.as_ref());

    let usages_result = match search_engine {
        SearchEngine::Ctags => {
            let results = search_tags(Path::new(&cwd), &identifier, &exact_or_inverse_terms);
            // tags might be incomplete, try the regex way if no results from the tags file.
            let try_regex =
                results.is_err() || results.as_ref().map(|r| r.is_empty()).unwrap_or(false);
            if try_regex {
                search_regex(identifier, extension, cwd, &exact_or_inverse_terms).await
            } else {
                results
            }
        }
        SearchEngine::Regex => {
            search_regex(identifier, extension, cwd, &exact_or_inverse_terms).await
        }
    };

    let (response, lines) = match usages_result {
        Ok(usages) => {
            let (lines, mut indices) = usages.deconstruct();
            let total_lines = lines;

            let response = {
                let total = total_lines.len();
                // Only show the top 200 items.
                let lines = total_lines.iter().take(200).collect::<Vec<_>>();
                indices.truncate(200);
                json!({
                  "id": msg_id,
                  "provider_id": "dumb_jump",
                  "force_execute": force_execute,
                  "result": { "lines": lines, "indices": indices, "total": total },
                })
            };

            (response, total_lines)
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
    SearchResults { lines, query }
}

#[derive(Debug, Clone, Default)]
pub struct DumbJumpMessageHandler {
    /// Last/Latest search results.
    results: SearchResults,
    /// Whether the tags file has been (re)-created.
    tags_regenerated: Arc<AtomicBool>,
}

impl DumbJumpMessageHandler {
    fn regenerate_tags(&mut self, dir: &str) {
        let tags_searcher = TagsSearcher::new(TagsConfig::with_dir(dir));
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
            .map_err(|_| anyhow::anyhow!("Missing `lnum` in {:?}", msg))?;

        // lnum is 1-indexed
        if let Some(curline) = self.results.lines.get((lnum - 1) as usize) {
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

        let job_future = if self.tags_regenerated.load(Ordering::Relaxed) {
            handle_dumb_jump_message(msg_id, params, SearchEngine::Ctags, false)
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
            move || handler.regenerate_tags(&dir)
        });
        tokio::spawn(async move {
            handle_dumb_jump_message(msg_id, params, SearchEngine::Regex, true).await;
        });

        Ok(session_sender)
    }
}
