use std::path::PathBuf;

use anyhow::Result;
use itertools::Itertools;
use structopt::StructOpt;

use filter::{
    matcher::{Bonus, MatchType},
    FilterContext, Source,
};

use crate::app::Params;
use crate::process::BaseCommand;
use crate::tools::ctags::{ensure_has_json_support, CtagsCommand};
use crate::utils::{send_response_from_cache, SendResponse};

const BASE_TAGS_CMD: &str = "ctags -R -x --output-format=json --fields=+n";

/// Generate ctags recursively given the directory.
#[derive(StructOpt, Debug, Clone)]
pub struct Tags {
    /// Initial query string
    #[structopt(index = 1, short, long)]
    query: String,

    /// The directory to generate recursive ctags.
    #[structopt(index = 2, short, long, parse(from_os_str))]
    dir: PathBuf,

    /// Specify the language.
    #[structopt(long = "languages")]
    languages: Option<String>,

    /// Read input from a cached grep tempfile, only absolute file path is supported.
    #[structopt(long = "input", parse(from_os_str))]
    input: Option<PathBuf>,

    /// Runs as the forerunner job, create the new cache entry.
    #[structopt(short, long)]
    forerunner: bool,

    /// Exclude files and directories matching 'pattern'.
    ///
    /// Will be translated into ctags' option: --exclude=pattern.
    #[structopt(long, default_value = ".git,*.json,node_modules,target,_build")]
    exclude: Vec<String>,
}

impl Tags {
    fn assemble_ctags_cmd(&self) -> CtagsCommand {
        let exclude = self
            .exclude
            .iter()
            .map(|x| x.split(',').collect::<Vec<_>>())
            .flatten()
            .map(|x| format!("--exclude={}", x))
            .join(" ");

        let mut command = format!("{} {}", BASE_TAGS_CMD, exclude);

        if let Some(ref languages) = self.languages {
            command.push_str(&format!(" --languages={}", languages));
        };

        CtagsCommand::new(BaseCommand::new(command, self.dir.clone()))
    }

    pub fn run(
        &self,
        Params {
            no_cache,
            icon_painter,
            ..
        }: Params,
    ) -> Result<()> {
        ensure_has_json_support()?;

        // In case of passing an invalid icon-painter option.
        let icon_painter = icon_painter.map(|_| icon::IconPainter::ProjTags);

        let ctags_cmd = self.assemble_ctags_cmd();

        if self.forerunner {
            let (total, cache) = if no_cache {
                ctags_cmd.create_cache()?
            } else if let Some((total, cache_path)) = ctags_cmd.get_ctags_cache() {
                (total, cache_path)
            } else {
                ctags_cmd.create_cache()?
            };
            send_response_from_cache(&cache, total, SendResponse::Json, icon_painter);
            return Ok(());
        } else {
            filter::dyn_run(
                &self.query,
                Source::List(ctags_cmd.formatted_tags_stream()?.map(Into::into)),
                FilterContext::new(None, Some(30), None, icon_painter, MatchType::TagName),
                vec![Bonus::None],
            )?;
        }

        Ok(())
    }
}
