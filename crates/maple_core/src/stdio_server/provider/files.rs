use crate::stdio_server::provider::{ClapProvider, Context, SearcherControl};
use anyhow::Result;
use clap::Parser;
use matcher::{Bonus, MatchScope};
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use types::Query;

use super::BaseArgs;

#[derive(Debug, Parser, PartialEq, Eq, Default)]
#[command(name = ":Clap files")]
#[command(about = "files provider", long_about = None)]
struct FilesArgs {
    #[clap(flatten)]
    base: BaseArgs,

    /// Whether to search hidden files.
    #[clap(long)]
    hidden: bool,

    /// Whether to match the file name only.
    #[clap(long)]
    name_only: bool,

    /// Specify additional search paths apart from the current working directory.
    #[clap(long = "path")]
    paths: Vec<PathBuf>,
}

#[derive(Debug)]
pub struct FilesProvider {
    args: FilesArgs,
    searcher_control: Option<SearcherControl>,
}

impl FilesProvider {
    pub async fn new(ctx: &Context) -> Result<Self> {
        let args: FilesArgs = ctx.parse_provider_args().await?;
        ctx.handle_base_args(&args.base).await?;

        let expanded_paths = ctx.expanded_paths(&args.paths).await?;
        Ok(Self {
            args: FilesArgs {
                paths: expanded_paths,
                ..args
            },
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
                let mut search_context = ctx.search_context(stop_signal.clone());
                if self.args.base.no_cwd {
                    search_context.paths = self.args.paths.clone();
                } else {
                    search_context.paths.extend_from_slice(&self.args.paths);
                }
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
                base: BaseArgs::default(),
                hidden: true,
                name_only: true,
                paths: vec![],
            }
        );

        assert_eq!(
            FilesArgs::parse_from(["", "--hidden"]),
            FilesArgs {
                base: BaseArgs::default(),
                hidden: true,
                name_only: false,
                paths: vec![],
            }
        );

        assert_eq!(
            FilesArgs::parse_from(["", "--name-only"]),
            FilesArgs {
                base: BaseArgs::default(),
                hidden: false,
                name_only: true,
                paths: vec![],
            }
        );

        assert_eq!(
            FilesArgs::parse_from(["", "--path=~", "--name-only"]),
            FilesArgs {
                base: BaseArgs::default(),
                hidden: false,
                name_only: true,
                paths: vec![PathBuf::from("~")],
            }
        );
    }
}
