use crate::stdio_server::handler::OnMoveImpl;
use crate::stdio_server::provider::{ClapProvider, ProviderContext, SearcherControl};
use crate::stdio_server::types::VimProgressor;
use crate::stdio_server::vim::Vim;
use anyhow::Result;
use matcher::{MatchScope, Matcher};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

fn start_searcher(number: usize, context: &ProviderContext, matcher: Matcher) -> SearcherControl {
    let stop_signal = Arc::new(AtomicBool::new(false));

    let join_handle = {
        let icon = context.env.icon;
        let winwidth = context.env.display_winwidth;
        let vim = context.vim.clone();
        let search_root = context.cwd.clone().into();
        let stop_signal = stop_signal.clone();

        tokio::spawn(async move {
            let progressor = VimProgressor::new(vim.clone(), stop_signal.clone());
            crate::searcher::Searcher {
                search_root,
                matcher,
                stop_signal,
                number,
                icon,
                winwidth,
                vim,
            }
            .run_with_progressor(progressor)
            .await
        })
    };

    SearcherControl {
        stop_signal,
        join_handle,
    }
}

#[derive(Debug)]
pub struct GrepProvider {
    context: ProviderContext,
    searcher_control: Option<SearcherControl>,
}

impl GrepProvider {
    pub fn new(context: ProviderContext) -> Self {
        Self {
            context,
            searcher_control: None,
        }
    }

    #[inline]
    fn vim(&self) -> &Vim {
        &self.context.vim
    }

    fn process_query(&mut self, query: String) {
        if let Some(control) = self.searcher_control.take() {
            tokio::task::spawn_blocking(move || {
                control.kill();
            });
        }

        let matcher = self
            .context
            .env
            .matcher_builder
            .clone()
            .match_scope(MatchScope::Full)
            .build(query.into());

        let new_control = start_searcher(100, &self.context, matcher);

        self.searcher_control.replace(new_control);
    }
}

#[async_trait::async_trait]
impl ClapProvider for GrepProvider {
    fn context(&self) -> &ProviderContext {
        &self.context
    }

    async fn on_create(&mut self) -> Result<()> {
        let query = self.vim().context_query_or_input().await?;
        if !query.is_empty() {
            self.process_query(query);
        }
        Ok(())
    }

    async fn on_move(&mut self) -> Result<()> {
        OnMoveImpl::new(&self.context, self.vim())
            .do_preview()
            .await
    }

    async fn on_typed(&mut self) -> Result<()> {
        let query = self.vim().input_get().await?;
        if query.is_empty() {
            self.vim().bare_exec("clap#state#clear_screen")?;
        } else {
            self.process_query(query);
        }
        Ok(())
    }

    fn on_terminate(&mut self, session_id: u64) {
        if let Some(control) = self.searcher_control.take() {
            // NOTE: The kill operation can not block current task.
            tokio::task::spawn_blocking(move || control.kill());
        }
        self.context.signify_terminated(session_id);
    }
}
