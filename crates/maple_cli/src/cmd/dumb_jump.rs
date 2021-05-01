//! Inspired by https://github.com/jacktasia/dumb-jump/blob/master/dumb-jump.el.
//!
//! This module requires the executable rg with `--json` and `--pcre2` is installed in the system.

use std::path::PathBuf;

use anyhow::Result;
use structopt::StructOpt;

use crate::dumb_analyzer::{
    find_occurrence_matches_by_ext, get_comments_by_ext, get_language_by_ext, DefinitionRules,
};
use crate::tools::ripgrep::{Match, Word};

/// All the lines as well as their match indices that can be sent to the vim side directly.
#[derive(Clone, Debug)]
pub struct Lines {
    pub lines: Vec<String>,
    pub indices: Vec<Vec<usize>>,
}

impl Lines {
    pub fn new(lines: Vec<String>, indices: Vec<Vec<usize>>) -> Self {
        Self { lines, indices }
    }

    pub fn print(&self) {
        let total = self.lines.len();
        let Self { lines, indices } = self;
        utility::println_json_with_length!(total, lines, indices);
    }
}

fn render(matches: Vec<Match>, kind: &str, word: &Word) -> Lines {
    let mut group_refs = std::collections::HashMap::new();

    // references are these occurrences not in the definitions.
    for line in matches.iter() {
        let counter = group_refs.entry(line.path()).or_insert_with(Vec::new);
        counter.push(line);
    }

    let mut res = Vec::new();

    for lines in group_refs.values() {
        res.push((
            format!("[{}] {} [{}]", kind, lines[0].path(), lines.len()),
            vec![],
        ));
        for line in lines {
            res.push(line.build_jump_line_bare("refs", word));
        }
    }

    let (lines, indices): (Vec<String>, Vec<Vec<usize>>) = res.into_iter().unzip();

    Lines::new(lines, indices)
}

fn render_jump_line(matches: Vec<Match>, kind: &str, word: &Word) -> Lines {
    let (lines, indices): (Vec<String>, Vec<Vec<usize>>) = matches
        .into_iter()
        .map(|line| line.build_jump_line(kind, &word))
        .unzip();

    Lines::new(lines, indices)
}

/// Execute the shell command
#[derive(StructOpt, Debug, Clone)]
pub struct DumbJump {
    /// Search term.
    #[structopt(index = 1, short, long)]
    pub word: String,

    /// File extension.
    #[structopt(index = 2, short, long)]
    pub extension: String,

    /// Definition kind.
    #[structopt(long = "kind")]
    pub kind: Option<String>,

    /// Specify the working directory.
    #[structopt(long = "cmd-dir", parse(from_os_str))]
    pub cmd_dir: Option<PathBuf>,
}

impl DumbJump {
    pub async fn run(self) -> Result<()> {
        let lang = get_language_by_ext(&self.extension)?;
        let comments = get_comments_by_ext(&self.extension);

        let word = Word::new(self.word.to_string())?;
        let res = DefinitionRules::definitions_and_references(
            lang,
            word.clone(),
            &self.cmd_dir,
            comments,
        )
        .await?;

        // render(res, "refs", &word).print();
        render_jump_line(res, "refs", &word).print();

        Ok(())
    }

    pub async fn references_or_occurrences(&self) -> Result<Lines> {
        let word = Word::new(self.word.to_string())?;

        let lang = match get_language_by_ext(&self.extension) {
            Ok(lang) => lang,
            Err(_) => {
                return Ok(render_jump_line(
                    find_occurrence_matches_by_ext(&word, &self.extension, &self.cmd_dir).await?,
                    "refs",
                    &word,
                ));
            }
        };

        let comments = get_comments_by_ext(&self.extension);
        let res = DefinitionRules::definitions_and_references(
            lang,
            word.clone(),
            &self.cmd_dir,
            comments,
        )
        .await?;

        // Ok(render_jump_line(res, "refs", &word))
        Ok(render(res, "refs", &word))
    }
}
