use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use crossbeam_channel::Sender;
use itertools::Itertools;
use log::error;
use serde::Deserialize;
use serde_json::json;

use filter::Query;

use crate::command::ctags::tagsfile::{Tags, TagsConfig};
use crate::command::dumb_jump::{DumbJump, Lines};
use crate::stdio_server::{
    event_handlers::OnMoveHandler,
    session::{Event, EventHandler, NewSession, Session, SessionContext, SessionEvent},
    write_response, Message,
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
async fn search_tags(dir: &PathBuf, query: &str) -> Result<Vec<String>> {
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

impl DumbJumpMessageHandler {
    async fn handle_dumb_jump_message(&mut self, msg: Message) {
        // TODO: try refilter

        let results = tokio::spawn(handle_dumb_jump_message(msg, false))
            .await
            .unwrap_or_else(|e| {
                log::error!(
                    "Failed to spawn a task for handle_dumb_jump_message: {:?}",
                    e
                );
                Default::default()
            });

        self.results = results;
    }
}

pub async fn handle_dumb_jump_message(msg: Message, force_execute: bool) -> SearchResults {
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
    } = msg.deserialize_params_unsafe();

    if query.is_empty() {
        return Default::default();
    }

    let last_query = query.clone();

    // When we use the dumb_jump, the search query should be `identifier(s) ++ exact_term/inverse_term`
    let Query {
        exact_terms,
        inverse_terms,
        fuzzy_terms,
    } = Query::from(query.as_str());

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
            (query, ExactOrInverseTerms::default())
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

    let dumb_jump = DumbJump {
        word: identifier,
        extension,
        kind: None,
        cmd_dir: Some(cwd.into()),
    };

    // TODO: not rerun the command but refilter the existing results if the query is just narrowed?
    let result = match dumb_jump
        .references_or_occurrences(false, &exact_or_inverse_terms)
        .await
    {
        Ok(Lines { lines, mut indices }) => {
            let total_lines = lines;
            let total = total_lines.len();
            // Only show the top 200 items.
            let lines = total_lines.iter().take(200).clone().collect::<Vec<_>>();
            indices.truncate(200);
            let result = json!({
            "lines": lines,
            "indices": indices,
            "total": total,
            });

            let result = json!({
              "id": msg_id,
              "force_execute": force_execute,
              "provider_id": "dumb_jump",
              "result": result,
            });

            write_response(result);

            return SearchResults {
                lines: total_lines,
                query: last_query,
            };
        }
        Err(e) => {
            error!("Error when running dumb_jump: {:?}", e);
            let error = json!({"message": e.to_string()});
            json!({ "id": msg_id, "provider_id": "dumb_jump", "error": error })
        }
    };

    write_response(result);

    SearchResults {
        lines: Default::default(),
        query: last_query,
    }
}

#[derive(Debug, Clone, Default)]
pub struct DumbJumpMessageHandler {
    /// Last/Latest search results.
    results: SearchResults,
}

#[async_trait::async_trait]
impl EventHandler for DumbJumpMessageHandler {
    async fn handle(&mut self, event: Event, context: Arc<SessionContext>) -> Result<()> {
        match event {
            Event::OnMove(msg) => {
                let msg_id = msg.id;

                let lnum = msg.get_u64("lnum").expect("lnum exists");

                // lnum is 1-indexed
                if let Some(curline) = self.results.lines.get((lnum - 1) as usize) {
                    if let Err(e) = OnMoveHandler::create(&msg, &context, Some(curline.into()))
                        .map(|x| x.handle())
                    {
                        log::error!("Failed to handle OnMove event: {:?}", e);
                        write_response(json!({"error": e.to_string(), "id": msg_id }));
                    }
                }
            }
            Event::OnTyped(msg) => self.handle_dumb_jump_message(msg).await,
        }
        Ok(())
    }
}

pub struct DumbJumpSession;

impl NewSession for DumbJumpSession {
    fn spawn(msg: Message) -> Result<Sender<SessionEvent>> {
        let (session, session_sender) =
            Session::new(msg.clone(), DumbJumpMessageHandler::default());

        session.start_event_loop()?;

        tokio::spawn(async move {
            handle_dumb_jump_message(msg, true).await;
        });

        Ok(session_sender)
    }
}
