use crate::stdio_server::handler::OnMoveImpl;
use crate::stdio_server::provider::{ClapProvider, ProviderContext, SearcherControl};
use crate::stdio_server::types::VimProgressor;
use anyhow::Result;
use matcher::{MatchScope, Matcher};
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

fn start_searcher(
    number: usize,
    context: &ProviderContext,
    search_root: PathBuf,
    hidden: bool,
    matcher: Matcher,
) -> SearcherControl {
    let stop_signal = Arc::new(AtomicBool::new(false));

    let join_handle = {
        let icon = context.env.icon;
        let winwidth = context.env.display_winwidth;
        let vim = context.vim.clone();
        let stop_signal = stop_signal.clone();

        tokio::spawn(async move {
            let progressor = VimProgressor::new(vim, stop_signal.clone());
            crate::searcher::files::FilesSearcher {
                search_root,
                hidden,
                matcher,
                stop_signal,
                number,
                icon,
                winwidth,
            }
            .run_with_progressor(progressor)
            .await;
        })
    };

    SearcherControl {
        stop_signal,
        join_handle,
    }
}

#[derive(Debug)]
pub struct FilesProvider {
    hidden: bool,
    name_only: bool,
    searcher_control: Option<SearcherControl>,
}

impl FilesProvider {
    pub async fn new(ctx: &ProviderContext) -> Result<Self> {
        let provider_args = ctx.vim.provider_args().await?;
        let hidden = provider_args.iter().any(|s| s == "--hidden");
        let name_only = ctx.vim.files_name_only().await?;
        Ok(Self {
            hidden,
            name_only,
            searcher_control: None,
        })
    }

    fn process_query(&mut self, query: String, ctx: &ProviderContext) {
        if let Some(control) = self.searcher_control.take() {
            tokio::task::spawn_blocking(move || {
                control.kill();
            });
        }

        let search_root = ctx.cwd.clone().into();

        let matcher_builder = ctx.env.matcher_builder.clone();

        let matcher_builder = if self.name_only {
            matcher_builder.match_scope(MatchScope::FileName)
        } else {
            matcher_builder.match_scope(MatchScope::Full)
        };

        let matcher = matcher_builder.build(query.into());

        let new_control = start_searcher(100, ctx, search_root, self.hidden, matcher);

        self.searcher_control.replace(new_control);
    }
}

#[async_trait::async_trait]
impl ClapProvider for FilesProvider {
    async fn on_create(&mut self, ctx: &mut ProviderContext) -> Result<()> {
        let query = ctx.vim.context_query_or_input().await?;
        if !query.is_empty() {
            self.process_query(query, ctx);
        }
        Ok(())
    }

    async fn on_move(&mut self, ctx: &mut ProviderContext) -> Result<()> {
        OnMoveImpl::new(ctx).do_preview().await
    }

    async fn on_typed(&mut self, ctx: &mut ProviderContext) -> Result<()> {
        let query = ctx.vim.input_get().await?;
        if query.is_empty() {
            ctx.vim.bare_exec("clap#state#clear_screen")?;
        } else {
            self.process_query(query, ctx);
        }
        Ok(())
    }

    fn on_terminate(&mut self, ctx: &mut ProviderContext, session_id: u64) {
        if let Some(control) = self.searcher_control.take() {
            // NOTE: The kill operation can not block current task.
            tokio::task::spawn_blocking(move || control.kill());
        }
        ctx.signify_terminated(session_id);
    }
}
