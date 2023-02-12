use crate::stdio_server::provider::{ClapProvider, Context, SearcherControl};
use anyhow::Result;
use matcher::MatchScope;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use types::Query;

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
        for args in &raw_args {
            let abs_path = ctx.vim.fnamemodify(args, ":p").await?;
            let abs_path = PathBuf::from(abs_path);
            if abs_path.is_absolute() {
                self.paths.push(abs_path);
            }
        }
        let query = ctx.vim.context_query_or_input().await?;
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
