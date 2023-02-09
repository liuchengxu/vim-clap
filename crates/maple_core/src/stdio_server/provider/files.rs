use crate::stdio_server::handler::initialize_provider;
use crate::stdio_server::provider::{ClapProvider, Context, SearcherControl};
use anyhow::Result;
use matcher::MatchScope;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use types::Query;

#[derive(Debug)]
pub struct FilesProvider {
    hidden: bool,
    name_only: bool,
    searcher_control: Option<SearcherControl>,
}

impl FilesProvider {
    pub async fn new(ctx: &Context) -> Result<Self> {
        let provider_args = ctx.vim.provider_args().await?;
        let hidden = provider_args.iter().any(|s| s == "--hidden");
        let name_only = ctx.vim.files_name_only().await?;
        Ok(Self {
            hidden,
            name_only,
            searcher_control: None,
        })
    }

    fn process_query(&mut self, query: String, ctx: &Context) {
        if let Some(control) = self.searcher_control.take() {
            tokio::task::spawn_blocking(move || {
                control.kill();
            });
        }

        let matcher = ctx
            .matcher_builder()
            .match_scope(if self.name_only {
                MatchScope::FileName
            } else {
                MatchScope::Full
            })
            .build(Query::from(&query));

        let new_control = {
            let stop_signal = Arc::new(AtomicBool::new(false));

            let join_handle = {
                let search_context = ctx.search_context(stop_signal.clone());
                let vim = ctx.vim.clone();
                let hidden = self.hidden;
                tokio::spawn(async move {
                    let _ = vim.bare_exec("clap#spinner#set_busy");
                    crate::searcher::files::search(query, hidden, matcher, search_context).await;
                    let _ = vim.bare_exec("clap#spinner#set_idle");
                })
            };

            SearcherControl {
                stop_signal,
                join_handle,
            }
        };

        self.searcher_control.replace(new_control);
    }
}

#[async_trait::async_trait]
impl ClapProvider for FilesProvider {
    async fn on_initialize(&mut self, ctx: &mut Context) -> Result<()> {
        let query = ctx.vim.context_query_or_input().await?;
        if !query.is_empty() {
            self.process_query(query, ctx);
        } else {
            initialize_provider(ctx).await?;
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
