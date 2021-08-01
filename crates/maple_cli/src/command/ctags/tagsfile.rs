use structopt::StructOpt;

use anyhow::Result;

use super::SharedParams;

use crate::{app::Params, process::BaseCommand};

/// Manipulate the tags file.
#[derive(StructOpt, Debug, Clone)]
pub struct TagsFile {
    /// Same with the `--kinds-all` option of ctags.
    #[structopt(long, default_value = "*")]
    kinds_all: String,

    /// Same with the `--fields` option of ctags.
    #[structopt(long, default_value = "*")]
    fields: String,

    /// Same with the `--extras` option of ctags.
    #[structopt(long, default_value = "*")]
    extras: String,

    /// Shared parameters arouns ctags.
    #[structopt(flatten)]
    shared: SharedParams,
}

impl TagsFile {
    pub fn run(&self, params: Params) -> Result<()> {
        // TODO: generate the output filepath according to the directory.
        let output = "/tmp/tags";

        // TODO: detect the languages by dir if not explicitly specified?
        let cmd = format!(
            "ctags --languages={} --kinds-all='{}' --fields='{}' --extras='{}' -f {} -R",
            self.shared.languages.as_ref().unwrap(),
            self.kinds_all,
            self.fields,
            self.extras,
            output
        );

        let ctags_cmd = BaseCommand::new(cmd, self.shared.dir.clone());

        let stdout_stream = filter::subprocess::Exec::shell(&ctags_cmd.command)
            .cwd(&ctags_cmd.cwd)
            .stream_stdout()?;

        use std::io::BufRead;
        let lines = std::io::BufReader::new(stdout_stream)
            .lines()
            .flatten()
            .collect::<Vec<_>>();

        println!("lines: {:?}", lines);

        Ok(())
    }
}
