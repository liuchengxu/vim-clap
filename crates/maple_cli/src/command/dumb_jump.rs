//! Inspired by https://github.com/jacktasia/dumb-jump/blob/master/dumb-jump.el.
//!
//! This module requires the executable rg with `--json` and `--pcre2` is installed in the system.

use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;

use crate::find_usages::{RegexSearcher, Usages};
use crate::utils::ExactOrInverseTerms;

/// Search-based jump.
#[derive(Parser, Debug, Clone)]
pub struct DumbJump {
    /// Search term.
    #[clap(index = 1, long)]
    pub word: String,

    /// File extension.
    #[clap(index = 2, long)]
    pub extension: String,

    /// Definition kind.
    #[clap(long)]
    pub kind: Option<String>,

    /// Specify the working directory.
    #[clap(long, parse(from_os_str))]
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
        Ok(searcher
            .search_usages(classify, exact_or_inverse_terms)
            .await?
            .into())
    }
}
