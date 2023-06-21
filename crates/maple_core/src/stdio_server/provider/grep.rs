use crate::stdio_server::provider::{ClapProvider, Context, SearcherControl};
use anyhow::Result;
use clap::Parser;
use matcher::MatchScope;
use serde_json::json;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use types::Query;

#[derive(Debug, Parser, PartialEq, Eq, Default)]
struct GrepArgs {
    #[clap(long)]
    query: String,
    #[clap(long)]
    path: Vec<PathBuf>,
}

#[derive(Debug)]
pub struct GrepProvider {
    paths: Vec<PathBuf>,
    searcher_control: Option<SearcherControl>,
}

impl GrepProvider {
    pub fn new() -> Self {
        Self {
            paths: Vec::new(),
            searcher_control: None,
        }
    }

    fn process_query(&mut self, query: String, ctx: &Context) {
        if let Some(control) = self.searcher_control.take() {
            tokio::task::spawn_blocking(move || {
                control.kill();
            });
        }

        let matcher = ctx
            .matcher_builder()
            .match_scope(MatchScope::Full) // Force using MatchScope::Full.
            .build(Query::from(&query));

        let new_control = {
            let stop_signal = Arc::new(AtomicBool::new(false));

            let vim = ctx.vim.clone();
            let mut search_context = ctx.search_context(stop_signal.clone());
            // cwd + extra paths
            search_context.paths.extend_from_slice(&self.paths);
            let join_handle = tokio::spawn(async move {
                let _ = vim.bare_exec("clap#spinner#set_busy");
                crate::searcher::grep::search(query, matcher, search_context).await;
                let _ = vim.bare_exec("clap#spinner#set_idle");
            });

            SearcherControl {
                stop_signal,
                join_handle,
            }
        };

        self.searcher_control.replace(new_control);
    }
}

#[async_trait::async_trait]
impl ClapProvider for GrepProvider {
    async fn on_initialize(&mut self, ctx: &mut Context) -> Result<()> {
        let raw_args = ctx.vim.provider_raw_args().await?;
        let GrepArgs { query, path } =
            GrepArgs::try_parse_from(std::iter::once(String::from("")).chain(raw_args.into_iter()))
                .map_err(|err| {
                    let _ = ctx.vim.echo_warn(format!(
                        "using default {:?} due to {err}",
                        GrepArgs::default()
                    ));
                })
                .unwrap_or_default();
        let query = if query.is_empty() {
            ctx.vim.input_get().await?
        } else {
            let query = match query.as_str() {
                "@visual" => ctx.vim.bare_call("clap#util#get_visual_selection").await?,
                _ => ctx.vim.call("clap#util#expand", json!([query])).await?,
            };
            ctx.vim.call("set_initial_query", json!([query])).await?;
            query
        };
        self.paths.extend(path);
        if !query.is_empty() {
            self.process_query(query, ctx);
        }
        Ok(())
    }

    async fn on_typed(&mut self, ctx: &mut Context) -> Result<()> {
        let query = ctx.vim.input_get().await?;
        if query.is_empty() {
            ctx.update_on_empty_query().await?;
        } else {
            self.process_query(query, ctx);
        }

        Ok(())
    }

    fn on_terminate(&mut self, ctx: &mut Context, session_id: u64) {
        if let Some(control) = self.searcher_control.take() {
            // NOTE: The kill operation can not block current task.
            tokio::task::spawn_blocking(move || control.kill());
        }
        ctx.signify_terminated(session_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_files_args() {
        assert_eq!(
            GrepArgs::parse_from(["", "--query=@visual", "--path=~/.vim/plugged/vim-clap"]),
            GrepArgs {
                query: String::from("@visual"),
                path: vec![PathBuf::from("~/.vim/plugged/vim-clap")]
            }
        );

        assert_eq!(
            GrepArgs::parse_from(["", "--query=@visual"]),
            GrepArgs {
                query: String::from("@visual"),
                path: vec![]
            }
        );

        assert_eq!(
            GrepArgs::parse_from([""]),
            GrepArgs {
                query: String::default(),
                path: vec![]
            }
        );
    }
}
