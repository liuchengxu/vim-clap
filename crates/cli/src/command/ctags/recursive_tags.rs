use super::CtagsCommonArgs;
use crate::app::Args;
use crate::{send_response_from_cache, SendResponse};
use anyhow::Result;
use clap::Parser;
use filter::{FilterContext, SequentialSource};
use itertools::Itertools;
use maple_core::process::ShellCommand;
use maple_core::tools::ctags::{ProjectCtagsCommand, CTAGS_BIN};
use matcher::{MatchScope, MatcherBuilder};
use rayon::prelude::*;
use std::sync::Arc;
use types::ClapItem;

/// Generate ctags recursively under the given directory.
#[derive(Parser, Debug, Clone)]
pub struct RecursiveTags {
    /// Query content.
    #[clap(long)]
    query: Option<String>,

    /// Runs as the forerunner job, create cache when neccessary.
    #[clap(long)]
    forerunner: bool,

    /// Run in parallel.
    #[clap(long)]
    par_run: bool,

    /// Ctags common arguments.
    #[clap(flatten)]
    pub(super) c_args: CtagsCommonArgs,
}

impl RecursiveTags {
    fn project_ctags_cmd(&self) -> Result<ProjectCtagsCommand> {
        let dir = self.c_args.dir()?;
        let exclude_args = self.c_args.exclude_args();

        let mut std_cmd = std::process::Command::new(ProjectCtagsCommand::TAGS_CMD[0]);
        std_cmd
            .current_dir(&dir)
            .args(&ProjectCtagsCommand::TAGS_CMD[1..])
            .args(exclude_args);
        if let Some(ref languages) = self.c_args.languages {
            std_cmd.arg(format!("--languages={languages}"));
        }

        let shell_cmd = std::iter::once(std_cmd.get_program())
            .chain(std_cmd.get_args())
            .map(|s| s.to_string_lossy())
            .join(" ");
        let shell_cmd = ShellCommand::new(shell_cmd, dir);

        Ok(ProjectCtagsCommand::new(std_cmd, shell_cmd))
    }

    pub fn run(
        &self,
        Args {
            no_cache,
            icon,
            number,
            ..
        }: Args,
    ) -> Result<()> {
        CTAGS_BIN.ensure_json_feature()?;

        let mut ctags_cmd = self.project_ctags_cmd()?;

        if self.forerunner {
            let (total, cache) = if no_cache {
                ctags_cmd.par_create_cache()?
            } else if let Some((total, cache_path)) = ctags_cmd.ctags_cache() {
                (total, cache_path)
            } else {
                ctags_cmd.par_create_cache()?
            };
            send_response_from_cache(&cache, total, SendResponse::Json, icon);
        } else {
            let filter_context = FilterContext::new(
                icon,
                number,
                None,
                MatcherBuilder::new().match_scope(MatchScope::TagName),
            );

            if self.par_run {
                filter::par_dyn_run_list(
                    self.query.as_deref().unwrap_or_default(),
                    filter_context,
                    ctags_cmd
                        .tag_item_iter()?
                        .map(|tag_item| Arc::new(tag_item) as Arc<dyn ClapItem>)
                        .par_bridge(),
                );
            } else {
                filter::dyn_run(
                    self.query.as_deref().unwrap_or_default(),
                    filter_context,
                    SequentialSource::Iterator(ctags_cmd.tag_item_iter()?.map(|tag_item| {
                        let item: Arc<dyn ClapItem> = Arc::new(tag_item);
                        item
                    })),
                )?;
            }
        }

        Ok(())
    }
}
