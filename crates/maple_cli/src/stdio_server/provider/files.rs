use crate::stdio_server::handler::{initialize_provider, OnMoveImpl};
use crate::stdio_server::provider::{ClapProvider, ProviderContext, SearcherControl};
use crate::stdio_server::types::VimProgressor;
use crate::stdio_server::vim::Vim;
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
    context: ProviderContext,
    hidden: bool,
    name_only: bool,
    searcher_control: Option<SearcherControl>,
}

impl FilesProvider {
    pub async fn new(context: ProviderContext) -> Result<Self> {
        let provider_args = context.vim.provider_args().await?;
        let hidden = provider_args.iter().any(|s| s == "--hidden");
        let name_only = context.vim.files_name_only().await?;
        Ok(Self {
            context,
            hidden,
            name_only,
            searcher_control: None,
        })
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

        let search_root = self.context.cwd.clone().into();

        let matcher_builder = self.context.env.matcher_builder.clone();

        let matcher_builder = if self.name_only {
            matcher_builder.match_scope(MatchScope::FileName)
        } else {
            matcher_builder.match_scope(MatchScope::Full)
        };

        let matcher = matcher_builder.build(query.into());

        let new_control = start_searcher(100, &self.context, search_root, self.hidden, matcher);

        self.searcher_control.replace(new_control);
    }
}

#[async_trait::async_trait]
impl ClapProvider for FilesProvider {
    fn context(&self) -> &ProviderContext {
        &self.context
    }

    async fn on_create(&mut self) -> Result<()> {
        let query = self.vim().context_query_or_input().await?;
        if !query.is_empty() {
            self.process_query(query);
        } else {
            initialize_provider(&self.context).await?;
        }
        Ok(())
    }

    async fn on_move(&mut self) -> Result<()> {
        if !self.context.env.preview_enabled {
            return Ok(());
        }
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
