use crate::stdio_server::handler::initialize_provider;
use crate::stdio_server::provider::{ClapProvider, Context, SearcherControl};
use anyhow::Result;
use matcher::{Bonus, MatchScope};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

#[derive(Debug)]
pub struct BlinesProvider {
    searcher_control: Option<SearcherControl>,
}

impl BlinesProvider {
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

        let source_file = ctx.env.start_buffer_path.clone();

        let matcher_builder = ctx.matcher_builder().match_scope(MatchScope::Full);

        let matcher = if let Some(extension) = source_file.extension().and_then(|s| s.to_str()) {
            matcher_builder
                .bonuses(vec![Bonus::Language(extension.into())])
                .build(query.into())
        } else {
            matcher_builder.build(query.into())
        };

        let new_control = {
            let stop_signal = Arc::new(AtomicBool::new(false));

            let join_handle = {
                let search_context = ctx.search_context(stop_signal.clone());

                tokio::spawn(async move {
                    crate::searcher::blines::search(source_file, matcher, search_context).await;
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
impl ClapProvider for BlinesProvider {
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
