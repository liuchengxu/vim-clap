use crate::app::Args;
use anyhow::{Context, Result};
use clap::Parser;
use maple_core::paths::AbsPathBuf;
use maple_core::tools::ctags::{buffer_tags_lines, current_context_tag};

/// Prints the tags for a specific file.
#[derive(Parser, Debug, Clone)]
pub struct BufferTags {
    /// Show the nearest function/method to a specific line.
    #[clap(long)]
    current_context: Option<usize>,

    /// Use the raw output format even json output is supported, for testing purpose.
    #[clap(long)]
    force_raw: bool,

    #[clap(long)]
    file: AbsPathBuf,
}

impl BufferTags {
    pub fn run(&self, _args: Args) -> Result<()> {
        if let Some(at) = self.current_context {
            let context_tag = current_context_tag(self.file.as_path(), at)
                .context("Error at finding the context tag info")?;
            println!("Context: {context_tag:?}");
            return Ok(());
        }

        let lines = buffer_tags_lines(self.file.as_ref(), self.force_raw)?;

        for line in lines {
            println!("{line}");
        }

        Ok(())
    }
}
