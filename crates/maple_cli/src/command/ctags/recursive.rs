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

impl RecursiveTags {
    fn assemble_ctags_cmd(&self) -> CtagsCommand {
        let exclude = self.shared.exclude_opt();

        let mut command = format!("{} {}", BASE_TAGS_CMD, exclude);

        if let Some(ref languages) = self.shared.languages {
            command.push_str(" --languages=");
            command.push_str(languages);
        };

        CtagsCommand::new(BaseCommand::new(command, self.shared.dir.clone()))
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
                if let Some(ref q) = self.query {
                    q
                } else {
                    Default::default()
                },
                Source::List(ctags_cmd.formatted_tags_stream()?.map(Into::into)),
                FilterContext::new(None, Some(30), None, icon_painter, MatchType::TagName),
                vec![Bonus::None],
            )?;
        }

        Ok(())
    }
}