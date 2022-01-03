//! Inspired by https://github.com/jacktasia/dumb-jump/blob/master/dumb-jump.el.
//!
//! This module requires the executable rg with `--json` and `--pcre2` is installed in the system.

use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::Result;
use rayon::prelude::*;
use structopt::StructOpt;

use crate::dumb_analyzer::{RegexSearcher, UsagesInfo};
use crate::tools::ripgrep::{Match, Word};
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
        /* FIXME
          let lang = get_language_by_ext(&self.extension)?;
          let comments = get_comments_by_ext(&self.extension);

          // TODO: also take word as query?
          let word = Word::new(self.word)?;

          definitions_and_references_lines(lang, &word, &self.cmd_dir, comments, &Default::default())
              .await?
              .print();
        */

        Ok(())
    }

    pub async fn references_or_occurrences(
        &self,
        classify: bool,
        exact_or_inverse_terms: &ExactOrInverseTerms,
    ) -> Result<UsagesInfo> {
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
