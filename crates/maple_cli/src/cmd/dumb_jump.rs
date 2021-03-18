//! Inspired by https://github.com/jacktasia/dumb-jump/blob/master/dumb-jump.el.

use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::{anyhow, Result};
use once_cell::sync::OnceCell;
use serde::Deserialize;
use structopt::StructOpt;

use crate::std_command::StdCommand;
use crate::tools::rg::{JsonLine, Word};

static RG_PCRE2_REGEX_RULES: OnceCell<HashMap<String, DefinitionRules>> = OnceCell::new();

static LANGUAGE_COMMENT_TABLE: OnceCell<HashMap<String, Vec<String>>> = OnceCell::new();

pub fn get_comments_by_ext(ext: &str) -> &[String] {
    let table = LANGUAGE_COMMENT_TABLE.get_or_init(|| {
        let comments: HashMap<String, Vec<String>> = serde_json::from_str(include_str!(
            "../../../../scripts/dumb_jump/comments_map.json"
        ))
        .unwrap();
        comments
    });

    table.get(ext).unwrap_or_else(|| table.get("*").unwrap())
}

/// Map of file extension to language.
///
/// NOTE: must be sorted as we use binary search to find if a key exists later.
///
/// https://github.com/BurntSushi/ripgrep/blob/20534fad04/crates/ignore/src/default_types.rs
pub const DEFAULT_LANGUAGE_EXT_TABLE: &[(&str, &str)] = &[
    ("clj", "clojure"),
    ("cpp", "cpp"),
    ("go", "go"),
    ("java", "java"),
    ("lua", "lua"),
    ("py", "python"),
    ("r", "r"),
    ("rb", "ruby"),
    ("rs", "rust"),
    ("scala", "scala"),
];

/// Finds the language given the file extension.
pub fn get_language_by_ext(ext: &str) -> Result<&str> {
    DEFAULT_LANGUAGE_EXT_TABLE
        .binary_search_by(|&(key, _)| key.cmp(&ext))
        .ok()
        .map(|idx| DEFAULT_LANGUAGE_EXT_TABLE[idx].1)
        .ok_or_else(|| anyhow!("dumb_jump is unsupported for {}", ext))
}

/// Unit type wrapper of the kind of definition.
///
/// Possibale values: variable, function, type, etc.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Hash)]
pub struct DefinitionKind(String);

impl AsRef<str> for DefinitionKind {
    fn as_ref(&self) -> &str {
        self.0.as_ref()
    }
}

/// Unit type wrapper of the regexp of a definition kind.
///
/// See more info in rg_pcre2_regex.json.
#[derive(Clone, Debug, Deserialize)]
pub struct DefinitionRegexp(Vec<String>);

impl DefinitionRegexp {
    pub fn iter(&self) -> impl Iterator<Item = &String> {
        self.0.iter()
    }
}

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

/// Definition rules of a language.
#[derive(Clone, Debug, Deserialize)]
pub struct DefinitionRules(HashMap<DefinitionKind, DefinitionRegexp>);

impl DefinitionRules {
    pub fn kind_rules_for(&self, kind: &DefinitionKind) -> Result<impl Iterator<Item = &str>> {
        self.0
            .get(kind)
            .ok_or_else(|| anyhow!("invalid definition kind {:?} for the rules", kind))
            .map(|x| x.iter().map(|x| x.as_str()))
    }

    pub fn build_full_regexp(lang: &str, kind: &DefinitionKind, word: &Word) -> Result<String> {
        let regexp = LanguageDefinition::get_rules(lang)?
            .kind_rules_for(kind)?
            .map(|x| x.replace("\\\\", "\\"))
            .map(|x| x.replace("JJJ", word.raw))
            .collect::<Vec<_>>()
            .join("|");
        Ok(regexp)
    }

    pub fn all_definitions(
        lang: &str,
        word: &Word,
        dir: &Option<PathBuf>,
    ) -> Result<Vec<(DefinitionKind, Vec<JsonLine>)>> {
        Ok(LanguageDefinition::get_rules(lang)?
            .0
            .iter()
            .filter_map(|(kind, _)| {
                find_definitions_in_jsonline(lang, kind, word, dir)
                    .ok()
                    .map(|line| (kind.clone(), line))
            })
            .collect())
    }

    pub fn definitions(lang: &str, word: &Word, dir: &Option<PathBuf>) -> Result<Lines> {
        let (lines, indices): (Vec<String>, Vec<Vec<usize>>) = LanguageDefinition::get_rules(lang)?
            .0
            .iter()
            .filter_map(|(kind, _)| find_definitions_per_kind(lang, kind, word, dir).ok())
            .flatten()
            .unzip();
        Ok(Lines::new(lines, indices))
    }

    pub fn definitions_and_references(
        lang: &str,
        word: &Word,
        dir: &Option<PathBuf>,
        comments: &[String],
    ) -> Result<Lines> {
        // TODO: run these tasks simultaneously
        let occurrences = find_all_occurrences_by_type(word, lang, dir, comments)?;
        let definitions = Self::all_definitions(lang, word, dir)?;

        let defs = definitions
            .iter()
            .map(|(_, defs)| defs)
            .flatten()
            .collect::<Vec<_>>();

        // There are some negative definitions we need to filter them out, e.g., the word
        // is a subtring in some identifer but we consider every word is a valid identifer.
        let positive_defs = defs
            .iter()
            .filter(|def| occurrences.contains(def))
            .collect::<Vec<_>>();

        let (lines, indices): (Vec<String>, Vec<Vec<usize>>) = definitions
            .iter()
            .flat_map(|(kind, lines)| {
                lines
                    .iter()
                    .filter(|line| positive_defs.contains(&line))
                    .map(|line| line.build_jump_line(kind.as_ref(), &word))
                    .collect::<Vec<_>>()
            })
            .chain(
                // references are these occurrences not in the definitions.
                occurrences
                    .iter()
                    .filter(|r| !defs.contains(&r))
                    .map(|line| line.build_jump_line("references", &word)),
            )
            .unzip();

        Ok(Lines::new(lines, indices))
    }
}

#[derive(Clone, Debug)]
pub struct LanguageDefinition;

impl LanguageDefinition {
    pub fn get_rules(lang: &str) -> Result<&DefinitionRules> {
        RG_PCRE2_REGEX_RULES
            .get_or_init(|| {
                let rules: HashMap<String, DefinitionRules> = serde_json::from_str(include_str!(
                    "../../../../scripts/dumb_jump/rg_pcre2_regex.json"
                ))
                .unwrap();
                rules
            })
            .get(lang)
            .ok_or_else(|| anyhow!("Language {} is unsupported in dumb-jump", lang))
    }
}

/// Executes the command as a child process, converting all the output into a stream of `JsonLine`.
fn collect_json_lines(
    command: String,
    dir: &Option<PathBuf>,
    comments: Option<&[String]>,
) -> Result<Vec<JsonLine>> {
    let mut std_cmd = StdCommand::new(command);

    if let Some(ref dir) = dir {
        std_cmd.current_dir(dir.to_path_buf());
    }

    let lines = std_cmd.lines()?;

    Ok(lines
        .iter()
        .filter_map(|s| serde_json::from_str::<JsonLine>(s).ok())
        .filter(|json_line| {
            if let Some(comments) = comments {
                !comments
                    .iter()
                    .any(|c| json_line.data.line().trim_start().starts_with(c))
            } else {
                true
            }
        })
        .collect())
}

/// Finds all the occurrences of `word`.
///
/// Basically the occurrences are composed of definitions and usages.
fn find_all_occurrences_by_type(
    word: &Word,
    lang_type: &str,
    dir: &Option<PathBuf>,
    comments: &[String],
) -> Result<Vec<JsonLine>> {
    let command = format!(
        "rg --json --word-regexp '{}' --type {}",
        word.raw, lang_type
    );

    collect_json_lines(command, dir, Some(comments))
}

fn find_occurrences_by_ext(word: &Word, ext: &str, dir: &Option<PathBuf>) -> Result<Lines> {
    let command = format!("rg --json --word-regexp '{}' -g '*.{}'", word.raw, ext);
    let comments = get_comments_by_ext(ext);
    let occurrences = collect_json_lines(command, dir, Some(comments))?;

    let (lines, indices) = occurrences
        .iter()
        .map(|line| line.build_jump_line("usages", word))
        .unzip();

    Ok(Lines::new(lines, indices))
}

fn find_definitions_per_kind(
    lang: &str,
    kind: &DefinitionKind,
    word: &Word,
    dir: &Option<PathBuf>,
) -> Result<Vec<(String, Vec<usize>)>> {
    let definitions = find_definitions_in_jsonline(lang, kind, word, dir)?;

    Ok(definitions
        .iter()
        .map(|line| line.build_jump_line(kind.as_ref(), word))
        .collect())
}

fn find_definitions_in_jsonline(
    lang: &str,
    kind: &DefinitionKind,
    word: &Word,
    dir: &Option<PathBuf>,
) -> Result<Vec<JsonLine>> {
    let regexp = DefinitionRules::build_full_regexp(lang, kind, word)?;
    let command = format!("rg --trim --json --pcre2 --type {} -e '{}'", lang, regexp);
    collect_json_lines(command, dir, None)
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
    pub fn run(&self) -> Result<()> {
        let lang = get_language_by_ext(&self.extension)?;
        let comments = get_comments_by_ext(&self.extension);

        let word = Word::new(&self.word);
        DefinitionRules::definitions_and_references(lang, &word, &self.cmd_dir, comments)?.print();

        Ok(())
    }

    pub fn references_or_occurrences(&self) -> Result<Lines> {
        let comments = get_comments_by_ext(&self.extension);

        let word = Word::new(&self.word);

        let lang = match get_language_by_ext(&self.extension) {
            Ok(lang) => lang,
            Err(_) => {
                return find_occurrences_by_ext(&word, &self.extension, &self.cmd_dir);
            }
        };

        DefinitionRules::definitions_and_references(lang, &word, &self.cmd_dir, comments)
    }
}
