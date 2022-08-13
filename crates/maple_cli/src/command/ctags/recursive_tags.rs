use std::ops::Deref;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use clap::Parser;

use filter::{FilterContext, Source};
use itertools::Itertools;
use matcher::{ClapItem, MatchScope, Matcher};
use rayon::prelude::*;

use super::SharedParams;
use crate::app::Params;
use crate::process::ShellCommand;
use crate::tools::ctags::{
    ProjectCtagsCommand, CTAGS_HAS_JSON_FEATURE, DEFAULT_EXCLUDE_OPT, EXCLUDE,
};
use crate::utils::{send_response_from_cache, SendResponse};

const TAGS_CMD: &[&str] = &["ctags", "-R", "-x", "--output-format=json", "--fields=+n"];
const BASE_TAGS_CMD: &str = "ctags -R -x --output-format=json --fields=+n";

/// Generate ctags recursively given the directory.
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

    /// Shared parameters arouns ctags.
    #[clap(flatten)]
    pub(super) shared: SharedParams,
}

pub fn build_recursive_ctags_cmd(cwd: PathBuf) -> ProjectCtagsCommand {
    let mut std_cmd = std::process::Command::new(TAGS_CMD[0]);
    std_cmd.current_dir(&cwd).args(&TAGS_CMD[1..]).args(
        EXCLUDE
            .split(',')
            .map(|exclude| format!("--exclude={exclude}")),
    );
    let shell_cmd = ShellCommand::new(
        format!("{} {}", BASE_TAGS_CMD, DEFAULT_EXCLUDE_OPT.deref()),
        cwd,
    );
    ProjectCtagsCommand::new(std_cmd, shell_cmd)
}

impl RecursiveTags {
    fn project_ctags_cmd(&self) -> Result<ProjectCtagsCommand> {
        let dir = self.shared.dir()?;
        let exclude_args = self.shared.exclude_args();

        let mut std_cmd = std::process::Command::new(TAGS_CMD[0]);
        std_cmd
            .current_dir(&dir)
            .args(&TAGS_CMD[1..])
            .args(&exclude_args);
        if let Some(ref languages) = self.shared.languages {
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
        Params {
            no_cache,
            icon,
            number,
            ..
        }: Params,
    ) -> Result<()> {
        if !CTAGS_HAS_JSON_FEATURE.deref() {
            return Err(anyhow::anyhow!(
                "ctags executable is not compiled with +json feature, please recompile it."
            ));
        }

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
                Matcher::default().set_match_scope(MatchScope::TagName),
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
                    Source::List(ctags_cmd.tag_item_iter()?.map(|tag_item| {
                        let item: Arc<dyn ClapItem> = Arc::new(tag_item);
                        item
                    })),
                )?;
            }
        }

        Ok(())
    }
}
