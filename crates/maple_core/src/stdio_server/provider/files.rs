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
struct FilesArgs {
    #[clap(flatten)]
    base: BaseArgs,

    /// Whether to search hidden files.
    #[clap(long)]
    hidden: bool,

    /// Whether to match the file name only.
    #[clap(long)]
    name_only: bool,

    #[clap(long)]
    path: Option<PathBuf>,
}

#[derive(Debug)]
pub struct FilesProvider {
    args: FilesArgs,
    searcher_control: Option<SearcherControl>,
}

impl FilesProvider {
    pub async fn new(ctx: &Context) -> Result<Self> {
        let mut args: FilesArgs = ctx.parse_provider_args().await?;
        ctx.handle_base_args(&args.base).await?;

        let mut ignore_path_arg = false;
        if let Some(path) = &args.path {
            if !path.try_exists().unwrap_or(false) {
                ignore_path_arg = true;
                let _ = ctx.vim.echo_warn(format!(
                    "Ignore `--path {:?}` as it does not exist",
                    path.display()
                ));
            }
        }

        if ignore_path_arg {
            args.path.take();
        }

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
                let mut search_context = ctx.search_context(stop_signal.clone());
                if let Some(dir) = &self.args.path {
                    search_context.paths = vec![dir.clone()];
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
                path: None,
                hidden: true,
                name_only: true
            }
        );

        assert_eq!(
            FilesArgs::parse_from(["", "--hidden"]),
            FilesArgs {
                base: BaseArgs::default(),
                path: None,
                hidden: true,
                name_only: false
            }
        );

        assert_eq!(
            FilesArgs::parse_from(["", "--name-only"]),
            FilesArgs {
                base: BaseArgs::default(),
                path: None,
                hidden: false,
                name_only: true
            }
        );

        assert_eq!(
            FilesArgs::parse_from(["", "--path=/Users", "--name-only"]),
            FilesArgs {
                base: BaseArgs::default(),
                path: Some(PathBuf::from("~")),
                hidden: false,
                name_only: true
            }
        );
    }
}
