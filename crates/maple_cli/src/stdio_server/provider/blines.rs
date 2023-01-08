use crate::stdio_server::handler::OnMoveImpl;
use crate::stdio_server::provider::{ClapProvider, Context, SearcherControl};
use crate::stdio_server::types::VimProgressor;
use anyhow::Result;
use matcher::{Bonus, MatchScope, Matcher};
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

fn start_searcher(
    number: usize,
    context: &Context,
    search_root: PathBuf,
    matcher: Matcher,
) -> SearcherControl {
    let stop_signal = Arc::new(AtomicBool::new(false));

    let join_handle = {
        let icon = context.env.icon;
        let display_winwidth = context.env.display_winwidth;
        let vim = context.vim.clone();
        let stop_signal = stop_signal.clone();

        tokio::spawn(async move {
            let progressor = VimProgressor::new(vim, stop_signal.clone());
            crate::searcher::blines::search(
                search_root,
                matcher,
                stop_signal,
                number,
                icon,
                display_winwidth,
                progressor,
            )
            .await;
        })
    };

    SearcherControl {
        stop_signal,
        join_handle,
    }
}

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

        let matcher_builder = ctx
            .env
            .matcher_builder
            .clone()
            .match_scope(MatchScope::Full);

        let matcher = if let Some(extension) = source_file.extension().and_then(|s| s.to_str()) {
            matcher_builder
                .bonuses(vec![Bonus::Language(extension.into())])
                .build(query.into())
        } else {
            matcher_builder.build(query.into())
        };

        let new_control = start_searcher(100, ctx, source_file, matcher);

        self.searcher_control.replace(new_control);
    }
}

#[async_trait::async_trait]
impl ClapProvider for BlinesProvider {
    async fn on_create(&mut self, ctx: &mut Context) -> Result<()> {
        let query = ctx.vim.context_query_or_input().await?;
        if !query.is_empty() {
            self.process_query(query, ctx);
        }
        Ok(())
    }

    async fn on_move(&mut self, ctx: &mut Context) -> Result<()> {
        OnMoveImpl::new(ctx).do_preview().await
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
