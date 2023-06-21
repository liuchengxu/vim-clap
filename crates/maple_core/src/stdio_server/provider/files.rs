use crate::stdio_server::provider::{ClapProvider, Context, SearcherControl};
use anyhow::Result;
use clap::Parser;
use matcher::{Bonus, MatchScope};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use types::Query;

#[derive(Debug, Parser, PartialEq, Eq, Default)]
struct FilesArgs {
    #[clap(long)]
    hidden: bool,
    #[clap(long)]
    name_only: bool,
}

#[derive(Debug)]
pub struct FilesProvider {
    args: FilesArgs,
    searcher_control: Option<SearcherControl>,
}

impl FilesProvider {
    pub async fn new(ctx: &Context) -> Result<Self> {
        let raw_args = ctx.vim.provider_raw_args().await?;
        let args =
            FilesArgs::try_parse_from(std::iter::once("".to_string()).chain(raw_args.into_iter()))
                .map_err(|err| {
                    let _ = ctx.vim.echo_warn(format!(
                        "using default {:?} due to {err}",
                        FilesArgs::default()
                    ));
                })
                .unwrap_or_default();
        Ok(Self {
            args,
            searcher_control: None,
        })
    }

    fn process_query(&mut self, query: String, ctx: &Context) {
        if let Some(control) = self.searcher_control.take() {
            tokio::task::spawn_blocking(move || {
                control.kill();
            });
        }

        let recent_files = crate::datastore::RECENT_FILES_IN_MEMORY
            .lock()
            .recent_n_files(50);
        let recent_files_bonus = Bonus::RecentFiles(recent_files.into());
        let matcher = ctx
            .matcher_builder()
            .match_scope(if self.args.name_only {
                MatchScope::FileName
            } else {
                MatchScope::Full
            })
            .bonuses(vec![recent_files_bonus])
            .build(Query::from(&query));

        let new_control = {
            let stop_signal = Arc::new(AtomicBool::new(false));

            let join_handle = {
                let search_context = ctx.search_context(stop_signal.clone());
                let vim = ctx.vim.clone();
                let hidden = self.args.hidden;
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
        // All files will be collected if query is empty
        self.process_query(query, ctx);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_files_args() {
        assert_eq!(
            FilesArgs::parse_from(["", "--hidden", "--name-only"]),
            FilesArgs {
                hidden: true,
                name_only: true
            }
        );

        assert_eq!(
            FilesArgs::parse_from(["", "--hidden"]),
            FilesArgs {
                hidden: true,
                name_only: false
            }
        );

        assert_eq!(
            FilesArgs::parse_from(["", "--name-only"]),
            FilesArgs {
                hidden: false,
                name_only: true
            }
        );
    }
}
