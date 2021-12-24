use std::path::PathBuf;

use anyhow::Result;
use structopt::StructOpt;

use filter::{
    matcher::{Bonus, MatchType},
    FilterContext, Source,
};

use super::SharedParams;

use crate::app::Params;
use crate::process::BaseCommand;
use crate::tools::ctags::{ensure_has_json_support, CtagsCommand};
use crate::utils::{send_response_from_cache, SendResponse};

const BASE_TAGS_CMD: &str = "ctags -R -x --output-format=json --fields=+n";

/// Generate ctags recursively given the directory.
#[derive(StructOpt, Debug, Clone)]
pub struct RecursiveTags {
    /// Query content.
    #[structopt(long)]
    query: Option<String>,

    /// Runs as the forerunner job, create cache when neccessary.
    #[structopt(long)]
    forerunner: bool,

    /// Shared parameters arouns ctags.
    #[structopt(flatten)]
    shared: SharedParams,
}

pub fn build_recursive_ctags_cmd(cwd: PathBuf) -> CtagsCommand {
    use itertools::Itertools;

    let exclude = super::EXCLUDE
        .split(',')
        .map(|x| format!("--exclude={}", x))
        .join(" ");

    let command = format!("{} {}", BASE_TAGS_CMD, exclude);

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

    pub fn run(&self, Params { no_cache, icon, .. }: Params) -> Result<()> {
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
            return Ok(());
        } else {
            filter::dyn_run(
                if let Some(ref q) = self.query {
                    q
                } else {
                    Default::default()
                },
                Source::List(ctags_cmd.formatted_tags_iter()?.map(Into::into)),
                FilterContext::new(Default::default(), icon, Some(30), None, MatchType::TagName),
                vec![Bonus::None],
            )?;
        }

        Ok(())
    }
}
