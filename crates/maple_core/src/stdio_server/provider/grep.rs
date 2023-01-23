use crate::stdio_server::provider::{ClapProvider, Context, SearcherControl};
use anyhow::Result;
use matcher::MatchScope;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

#[derive(Debug)]
pub struct GrepProvider {
    searcher_control: Option<SearcherControl>,
}

impl GrepProvider {
    pub fn new() -> Self {
        Self {
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
            .match_scope(MatchScope::Full)
            .build(query.into());

        let new_control = {
            let stop_signal = Arc::new(AtomicBool::new(false));

            let search_context = ctx.search_context(stop_signal.clone());
            let join_handle = tokio::spawn(async move {
                crate::searcher::grep::search(matcher, search_context).await
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
        let query = ctx.vim.context_query_or_input().await?;
        if !query.is_empty() {
            self.process_query(query, ctx);
        }
        Ok(())
    }

    async fn on_typed(&mut self, ctx: &mut Context) -> Result<()> {
        let query = ctx.vim.input_get().await?;
        if query.is_empty() {
            ctx.vim.bare_exec("clap#state#clear_screen")?;
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
