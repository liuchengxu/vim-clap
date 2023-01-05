use crate::stdio_server::provider::{ClapProvider, ProviderContext, SearcherControl};
use crate::stdio_server::types::VimProgressor;
use crate::stdio_server::vim::Vim;
use anyhow::Result;
use matcher::{Bonus, MatchScope, Matcher};
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

fn start_searcher(
    number: usize,
    context: &ProviderContext,
    vim: Vim,
    search_root: PathBuf,
    matcher: Matcher,
) -> SearcherControl {
    let stop_signal = Arc::new(AtomicBool::new(false));

    let join_handle = {
        let icon = context.env.icon;
        let display_winwidth = context.env.display_winwidth;
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
    context: ProviderContext,
    searcher_control: Option<SearcherControl>,
}

impl BlinesProvider {
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

        let source_file = self.context.env.start_buffer_path.clone();

        let matcher_builder = self
            .context
            .env
            .matcher_builder
            .clone()
            .match_scope(MatchScope::Full);

        let matcher = if let Some(extension) = source_file
            .extension()
            .and_then(|s| s.to_str().map(|s| s.to_string()))
        {
            matcher_builder
                .bonuses(vec![Bonus::Language(extension.into())])
                .build(query.into())
        } else {
            matcher_builder.build(query.into())
        };

        let new_control =
            start_searcher(100, &self.context, self.vim().clone(), source_file, matcher);

        self.searcher_control.replace(new_control);
    }
}

#[async_trait::async_trait]
impl ClapProvider for BlinesProvider {
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
        crate::stdio_server::handler::OnMoveImpl::new(&self.context, self.vim())
            .do_preview()
            .await
    }

    async fn on_typed(&mut self) -> Result<()> {
        let query = self.vim().input_get().await?;
        self.process_query(query);
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
