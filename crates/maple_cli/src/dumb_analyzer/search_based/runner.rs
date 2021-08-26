use std::convert::TryFrom;
use std::path::PathBuf;

use anyhow::Result;

use crate::process::AsyncCommand;
use crate::tools::ripgrep::{Match, Word};

use super::definition::{get_comments_by_ext, is_comment, DefinitionKind, DefinitionRules};

/// Executes `command` as a child process.
///
/// Convert the entire output into a stream of ripgrep `Match`.
async fn collect_matches(
    command: String,
    dir: &Option<PathBuf>,
    comments: Option<&[&str]>,
) -> Result<Vec<Match>> {
    let mut cmd = AsyncCommand::new(command);

    if let Some(ref dir) = dir {
        cmd.current_dir(dir);
    }

    if let Some(comments) = comments {
        cmd.execute_and_filter_map(|s| {
            Match::try_from(s)
                .ok()
                .filter(|mat| !is_comment(&mat, comments))
        })
        .await
    } else {
        cmd.execute_and_filter_map(|s| Match::try_from(s).ok())
            .await
    }
}

/// Finds all the occurrences of `word`.
///
/// Basically the occurrences are composed of definitions and usages.
pub async fn find_all_occurrences_by_type(
    word: &Word,
    lang_type: &str,
    dir: &Option<PathBuf>,
    comments: &[&str],
) -> Result<Vec<Match>> {
    let command = format!(
        "rg --json --word-regexp '{}' --type {}",
        word.raw, lang_type
    );

    collect_matches(command, dir, Some(comments)).await
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
    collect_matches(command, dir, Some(comments)).await
}

pub async fn find_occurrence_matches_by_ext(
    word: &Word,
    ext: &str,
    dir: &Option<PathBuf>,
) -> Result<Vec<Match>> {
    let command = format!("rg --json --word-regexp '{}' -g '*.{}'", word.raw, ext);
    let comments = get_comments_by_ext(ext);
    let occurrences = collect_matches(command, dir, Some(comments)).await?;

    Ok(occurrences)
}

#[allow(unused)]
pub async fn find_definitions_matches(
    lang: &str,
    kind: &DefinitionKind,
    word: &Word,
    dir: &Option<PathBuf>,
) -> Result<Vec<Match>> {
    let regexp = DefinitionRules::build_full_regexp(lang, kind, word)?;
    let command = format!("rg --trim --json --pcre2 --type {} -e '{}'", lang, regexp);
    collect_matches(command, dir, None).await
}

pub async fn find_definition_matches_with_kind(
    lang: &str,
    kind: &DefinitionKind,
    word: &Word,
    dir: &Option<PathBuf>,
) -> Result<(DefinitionKind, Vec<Match>)> {
    let regexp = DefinitionRules::build_full_regexp(lang, kind, word)?;
    let command = format!("rg --trim --json --pcre2 --type {} -e '{}'", lang, regexp);
    collect_matches(command, dir, None)
        .await
        .map(|defs| (kind.clone(), defs))
}
