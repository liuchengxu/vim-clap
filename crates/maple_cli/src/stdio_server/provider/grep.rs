use crate::stdio_server::provider::{
    start_searcher, ClapProvider, ProviderContext, SearcherControl,
};
use crate::stdio_server::vim::Vim;
use anyhow::Result;
use matcher::MatchScope;

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

        let new_control = start_searcher(
            100,
            &self.context,
            self.vim().clone(),
            self.context.cwd.clone().into(),
            self.context
                .env
                .matcher_builder
                .clone()
                .match_scope(MatchScope::Full)
                .build(query.into()),
        );

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
