use std::hash::Hash;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use anyhow::Result;
use itertools::Itertools;
use structopt::StructOpt;

use filter::{
    matcher::{Bonus, MatchType},
    subprocess, FilterContext, Source,
};

use crate::app::Params;
use crate::cache::{send_response_from_cache, SendResponse};
use crate::process::BaseCommand;
use crate::tools::ctags::{ensure_has_json_support, CtagsCommand, TagInfo};

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

fn create_tags_cache<T: AsRef<Path> + Clone + Hash>(
    args: &[&str],
    dir: T,
) -> Result<(PathBuf, usize)> {
    todo!()
    /*
    let tags_stream = formatted_tags_stream(args, dir.clone())?;
    let mut total = 0usize;
    let mut formatted_tags_stream = tags_stream.map(|x| {
        total += 1;
        x
    });
    let lines = formatted_tags_stream.join("\n");
    todo!("Create cache for tags")
    */
    // let cache = CacheEntry::create(args, Some(dir), total, lines)?;
    // Ok((cache, total))
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
            // TODO:
            // let (cache, total) = if no_cache {
            // create_tags_cache(&cmd_args, &self.dir)?
            // } else if let Ok(cached_info) = cache_exists(&cmd_args, &self.dir) {
            // cached_info
            // } else {
            // create_tags_cache(&cmd_args, &self.dir)?
            // };
            // send_response_from_cache(&cache, total, SendResponse::Json, icon_painter);
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
