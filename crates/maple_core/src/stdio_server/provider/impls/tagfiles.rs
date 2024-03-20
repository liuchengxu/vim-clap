use crate::stdio_server::provider::hooks::initialize_provider;
use crate::stdio_server::provider::{
    BaseArgs, ClapProvider, Context, ProviderResult as Result, SearcherControl,
};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use types::Query;

#[derive(Debug)]
pub struct TagfilesProvider {
    args: BaseArgs,
    searcher_control: Option<SearcherControl>,
}

impl TagfilesProvider {
    pub async fn new(ctx: &Context) -> Result<Self> {
        let args: BaseArgs = ctx.parse_provider_args().await?;
        Ok(Self {
            args,
            searcher_control: None,
        })
    }

    fn process_query(&mut self, query: String, ctx: &Context) {
        if let Some(control) = self.searcher_control.take() {
            control.kill_in_background();
        }

        let matcher = ctx.matcher_builder().build(Query::from(&query));

        let new_control = {
            let stop_signal = Arc::new(AtomicBool::new(false));

            let join_handle = {
                let search_context = ctx.search_context(stop_signal.clone());
                let cwd = ctx.cwd.to_path_buf();

                tokio::spawn(async move {
                    crate::searcher::tagfiles::search(query, cwd, matcher, search_context).await;
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
impl ClapProvider for TagfilesProvider {
    async fn on_initialize(&mut self, ctx: &mut Context) -> Result<()> {
        if self.args.query.is_none() {
            initialize_provider(ctx, false).await?;
        }

        ctx.handle_base_args(&self.args).await?;

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

    async fn on_move(&mut self, _ctx: &mut Context) -> Result<()> {
        // TODO: Possible to include the line number in tagfiles?
        Ok(())
    }

    fn on_terminate(&mut self, ctx: &mut Context, session_id: u64) {
        if let Some(control) = self.searcher_control.take() {
            control.kill_in_background();
        }
        ctx.signify_terminated(session_id);
    }
}
