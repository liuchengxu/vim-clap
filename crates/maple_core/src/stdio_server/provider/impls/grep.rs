use crate::stdio_server::provider::{
    BaseArgs, ClapProvider, Context, ProviderResult as Result, SearcherControl,
};
use clap::Parser;
use matcher::MatchScope;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use types::Query;

#[derive(Debug, Parser, PartialEq, Eq, Default)]
#[command(name = ":Clap grep")]
#[command(about = "grep provider", long_about = None)]
struct GrepArgs {
    #[clap(flatten)]
    base: BaseArgs,

    /// Specify additional search paths apart from the current working directory.
    #[clap(long = "path")]
    paths: Vec<PathBuf>,
}

#[derive(Debug)]
pub struct GrepProvider {
    args: GrepArgs,
    searcher_control: Option<SearcherControl>,
}

impl GrepProvider {
    pub async fn new(ctx: &Context) -> Result<Self> {
        let GrepArgs { base, paths } = ctx.parse_provider_args().await?;
        Ok(Self {
            args: GrepArgs {
                base,
                paths: ctx.expanded_paths(&paths).await?,
            },
            searcher_control: None,
        })
    }

    fn process_query(&mut self, query: String, ctx: &Context) {
        if let Some(control) = self.searcher_control.take() {
            control.kill_in_background();
        }

        let matcher = ctx
            .matcher_builder()
            .match_scope(MatchScope::Full) // Force using MatchScope::Full.
            .build(Query::from(&query));

        let new_control = {
            let stop_signal = Arc::new(AtomicBool::new(false));

            let vim = ctx.vim.clone();
            let mut search_context = ctx.search_context(stop_signal.clone());
            // cwd + extra paths
            if self.args.base.no_cwd {
                search_context.paths = self.args.paths.clone();
            } else {
                search_context.paths.extend_from_slice(&self.args.paths);
            }
            let join_handle = tokio::spawn(async move {
                let _ = vim.bare_exec("clap#spinner#set_busy");
                crate::searcher::grep::search(query, matcher, search_context).await;
                let _ = vim.bare_exec("clap#spinner#set_idle");
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
        ctx.handle_base_args(&self.args.base).await
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
            control.kill_in_background();
        }
        ctx.signify_terminated(session_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_files_args() {
        assert_eq!(
            GrepArgs::parse_from(["", "--query=@visual", "--path=~/.vim/plugged/vim-clap"]),
            GrepArgs {
                base: BaseArgs {
                    query: Some(String::from("@visual")),
                    ..Default::default()
                },
                paths: vec![PathBuf::from("~/.vim/plugged/vim-clap")]
            }
        );

        assert_eq!(
            GrepArgs::parse_from(["", "--query=@visual"]),
            GrepArgs {
                base: BaseArgs {
                    query: Some(String::from("@visual")),
                    ..Default::default()
                },
                paths: vec![]
            }
        );

        assert_eq!(
            GrepArgs::parse_from([""]),
            GrepArgs {
                base: BaseArgs::default(),
                paths: vec![]
            }
        );
    }
}
