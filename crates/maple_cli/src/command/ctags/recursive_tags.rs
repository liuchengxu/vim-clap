use std::ops::Deref;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use clap::Parser;

use filter::{FilterContext, Source};
use matcher::{ClapItem, MatchScope, Matcher};
use rayon::prelude::*;

use super::SharedParams;
use crate::app::Params;
use crate::process::BaseCommand;
use crate::tools::ctags::{ensure_has_json_support, CtagsCommand, TagInfo, DEFAULT_EXCLUDE_OPT};
use crate::utils::{send_response_from_cache, SendResponse};

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

pub fn build_recursive_ctags_cmd(cwd: PathBuf) -> CtagsCommand {
    let command = format!("{} {}", BASE_TAGS_CMD, DEFAULT_EXCLUDE_OPT.deref());

    CtagsCommand::new(BaseCommand::new(command, cwd))
}

impl RecursiveTags {
    fn assemble_ctags_cmd(&self) -> Result<CtagsCommand> {
        let exclude = self.shared.exclude_opt();

        let mut command = format!("{} {}", BASE_TAGS_CMD, exclude);

        if let Some(ref languages) = self.shared.languages {
            command.push_str(" --languages=");
            command.push_str(languages);
        };

        Ok(CtagsCommand::new(BaseCommand::new(
            command,
            self.shared.dir()?,
        )))
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
        ensure_has_json_support()?;

        let ctags_cmd = self.assemble_ctags_cmd()?;

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
                        .stdout()?
                        .par_split(|x| x == &b'\n')
                        .filter_map(|line| {
                            if let Ok(tag_info) = serde_json::from_slice::<TagInfo>(line) {
                                let item: Arc<dyn ClapItem> = Arc::new(tag_info.into_tag_item());
                                Some(item)
                            } else {
                                None
                            }
                        }),
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
