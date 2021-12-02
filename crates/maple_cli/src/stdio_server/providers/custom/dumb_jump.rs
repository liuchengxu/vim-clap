use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::Result;
use crossbeam_channel::Sender;
use itertools::Itertools;
use serde::Deserialize;
use serde_json::json;

use filter::Query;

use crate::command::ctags::tagsfile::{Tags, TagsConfig};
use crate::command::dumb_jump::{DumbJump, Lines};
use crate::stdio_server::{
    providers::builtin::OnMoveHandler,
    rpc::Call,
    session::{EventHandler, NewSession, Session, SessionContext, SessionEvent},
    write_response, MethodCall,
};
use crate::utils::ExactOrInverseTerms;

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

#[allow(unused)]
async fn search_tags(dir: &Path, query: &str) -> Result<Vec<String>> {
    let tags = Tags::new(TagsConfig::with_dir(dir));
    if tags.exists() {
        for line in tags.readtags(query)?.collect::<Vec<_>>() {
            println!("{}", line);
        }
        todo!()
    } else {
        Ok(Default::default())
    }
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

pub async fn handle_dumb_jump_message(msg: MethodCall, force_execute: bool) -> SearchResults {
    let msg_id = msg.id;

    #[derive(Deserialize)]
    struct Params {
        cwd: String,
        query: String,
        extension: String,
    }

    let Params {
        cwd,
        query,
        extension,
    } = msg.parse_unsafe();

    if query.is_empty() {
        return Default::default();
    }

    let (identifier, exact_or_inverse_terms) = parse_raw_query(query.as_ref());

    let dumb_jump = DumbJump {
        word: identifier,
        extension,
        kind: None,
        cmd_dir: Some(cwd.into()),
    };

    // TODO: not rerun the command but refilter the existing results if the query is just narrowed?
    match dumb_jump
        .references_or_occurrences(false, &exact_or_inverse_terms)
        .await
    {
        Ok(Lines { lines, mut indices }) => {
            let total_lines = lines;
            let total = total_lines.len();
            // Only show the top 200 items.
            let lines = total_lines.iter().take(200).collect::<Vec<_>>();
            indices.truncate(200);

            let result = json!({
              "id": msg_id,
              "force_execute": force_execute,
              "provider_id": "dumb_jump",
              "result": { "lines": lines, "indices": indices, "total": total },
            });

            write_response(result);
            SearchResults {
                lines: total_lines,
                query,
            }
        }
        Err(e) => {
            tracing::error!(error = ?e, "Error when running dumb_jump");
            let result = json!({
                "id": msg_id,
                "provider_id": "dumb_jump",
                "error": { "message": e.to_string() }
            });
            write_response(result);
            SearchResults {
                lines: Default::default(),
                query,
            }
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct DumbJumpMessageHandler {
    /// Last/Latest search results.
    results: SearchResults,
}

#[async_trait::async_trait]
impl EventHandler for DumbJumpMessageHandler {
    async fn handle_on_move(
        &mut self,
        msg: MethodCall,
        context: Arc<SessionContext>,
    ) -> Result<()> {
        let msg_id = msg.id;

        let lnum = msg.get_u64("lnum").expect("lnum exists");

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
        let results = tokio::spawn(handle_dumb_jump_message(msg, false))
            .await
            .unwrap_or_else(|e| {
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
        let (session, session_sender) =
            Session::new(call.clone(), DumbJumpMessageHandler::default());

        session.start_event_loop();

        tokio::spawn(async move {
            handle_dumb_jump_message(call.unwrap_method_call(), true).await;
        });

        Ok(session_sender)
    }
}
