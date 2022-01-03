//! Inspired by https://github.com/jacktasia/dumb-jump/blob/master/dumb-jump.el.
//!
//! This module requires the executable rg with `--json` and `--pcre2` is installed in the system.

use std::path::PathBuf;

use anyhow::Result;
use structopt::StructOpt;

use crate::dumb_analyzer::{RegexSearcher, Usages};
use crate::utils::ExactOrInverseTerms;

/// Search-based jump.
#[derive(StructOpt, Debug, Clone)]
pub struct DumbJump {
    /// Search term.
    #[structopt(index = 1, long)]
    pub word: String,

    /// File extension.
    #[structopt(index = 2, long)]
    pub extension: String,

    /// Definition kind.
    #[structopt(long)]
    pub kind: Option<String>,

    /// Specify the working directory.
    #[structopt(long, parse(from_os_str))]
    pub cmd_dir: Option<PathBuf>,
}

impl DumbJump {
    pub async fn run(self) -> Result<()> {
        RegexSearcher {
            word: self.word.to_string(),
            extension: self.extension.to_string(),
            dir: self.cmd_dir.clone(),
        }
        .print_usages(&Default::default())
        .await
    }

    pub async fn references_or_occurrences(
        &self,
        classify: bool,
        exact_or_inverse_terms: &ExactOrInverseTerms,
    ) -> Result<Usages> {
        let searcher = RegexSearcher {
            word: self.word.to_string(),
            extension: self.extension.to_string(),
            dir: self.cmd_dir.clone(),
        };
        searcher
            .search_usages(classify, exact_or_inverse_terms)
            .await
    }
}
