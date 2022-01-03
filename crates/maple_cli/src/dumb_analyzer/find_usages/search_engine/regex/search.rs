use std::convert::TryFrom;
use std::path::PathBuf;

use anyhow::Result;
use rayon::prelude::*;

use crate::process::AsyncCommand;
use crate::tools::ripgrep::{Match, Word};

use super::definition::{build_full_regexp, get_comments_by_ext, is_comment, DefinitionKind};

/// Executes `command` as a child process.
///
/// Convert the entire output into a stream of ripgrep `Match`.
async fn find_matches(
    command: String,
    dir: &Option<PathBuf>,
    comments: Option<&[&str]>,
) -> Result<Vec<Match>> {
    let mut cmd = AsyncCommand::new(command);

    if let Some(ref dir) = dir {
        cmd.current_dir(dir);
    }

    let stdout = cmd.stdout()?;

    if let Some(comments) = comments {
        Ok(stdout
            .par_split(|x| x == &b'\n')
            .filter_map(|s| {
                Match::try_from(s)
                    .ok()
                    .filter(|mat| !is_comment(mat, comments))
            })
            .collect())
    } else {
        Ok(stdout
            .par_split(|x| x == &b'\n')
            .filter_map(|s| Match::try_from(s).ok())
            .collect())
    }
}

pub async fn naive_grep_fallback(
    word: &Word,
    lang_type: &str,
    dir: &Option<PathBuf>,
    comments: &[&str],
) -> Result<Vec<Match>> {
    let command = format!(
        "rg --json -e '{}' --type {}",
        word.raw.replace(char::is_whitespace, ".*"),
        lang_type
    );
    find_matches(command, dir, Some(comments)).await
}

pub async fn find_occurrence_matches_by_ext(
    word: &Word,
    ext: &str,
    dir: &Option<PathBuf>,
) -> Result<Vec<Match>> {
    let command = format!("rg --json --word-regexp '{}' -g '*.{}'", word.raw, ext);
    let comments = get_comments_by_ext(ext);
    find_matches(command, dir, Some(comments)).await
}

/// Finds all the occurrences of `word`.
///
/// Basically the occurrences are composed of definitions and usages.
pub async fn find_occurrences_by_lang(
    word: &Word,
    lang_type: &str,
    dir: &Option<PathBuf>,
    comments: &[&str],
) -> Result<Vec<Match>> {
    let command = format!(
        "rg --json --word-regexp '{}' --type {}",
        word.raw, lang_type
    );

    find_matches(command, dir, Some(comments)).await
}

/// Returns a tuple of (definition_kind, ripgrep_matches) by searching given language `lang`.
pub async fn find_definition_matches_with_kind(
    lang: &str,
    kind: &DefinitionKind,
    word: &Word,
    dir: &Option<PathBuf>,
) -> Result<(DefinitionKind, Vec<Match>)> {
    let regexp = build_full_regexp(lang, kind, word)?;
    let command = format!("rg --trim --json --pcre2 --type {} -e '{}'", lang, regexp);
    find_matches(command, dir, None)
        .await
        .map(|defs| (kind.clone(), defs))
}
