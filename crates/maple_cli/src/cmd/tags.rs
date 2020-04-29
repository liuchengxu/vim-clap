use crate::cmd::cache::{cache_exists, send_response_from_cache, CacheEntry, SendResponse};
use crate::ContentFiltering;
use anyhow::Result;
use fuzzy_filter::{subprocess, Source};
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use structopt::StructOpt;

const BASE_TAGS_ARGS: [&str; 9] = [
    "ctags",
    "-R",
    "-x",
    "--output-format=json",
    "--fields=+n",
    "--exclude=.git",
    "--exclude=*.json",
    "--exclude=node_modules",
    "--exclude=target",
];

#[derive(Serialize, Deserialize, Debug)]
struct TagInfo {
    name: String,
    path: String,
    pattern: String,
    line: usize,
    kind: String,
}

impl TagInfo {
    pub fn format(&self) -> String {
        let pat_len = self.pattern.len();
        let name_lnum = format!("{}:{}", self.name, self.line);
        let kind = format!("[{}@{}]", self.kind, self.path);
        format!(
            "{text:<width1$} {kind:<width2$} {pattern}",
            text = name_lnum,
            width1 = 30,
            kind = kind,
            width2 = 30,
            pattern = &self.pattern[2..pat_len - 2].trim(),
        )
    }
}

/// Generate ctags recursively given the directory.
#[derive(StructOpt, Debug, Clone)]
pub struct Tags {
    /// Initial query string
    #[structopt(index = 1, short, long)]
    query: String,

    /// The directory to generate recursive ctags.
    #[structopt(index = 2, short, long, parse(from_os_str))]
    dir: PathBuf,

    /// Specify the language.
    #[structopt(long = "languages")]
    languages: Option<String>,

    /// Read input from a cached grep tempfile, only absolute file path is supported.
    #[structopt(long = "input", parse(from_os_str))]
    input: Option<PathBuf>,

    /// Runs as the forerunner job, create the new cache entry.
    #[structopt(short, long)]
    forerunner: bool,
}

fn formatted_tags_stream(args: &[&str], dir: &PathBuf) -> Result<impl Iterator<Item = String>> {
    let stdout_stream = subprocess::Exec::shell(args.join(" "))
        .cwd(dir)
        .stream_stdout()?;
    Ok(BufReader::new(stdout_stream).lines().filter_map(|line| {
        line.ok().and_then(|tag| {
            if let Ok(tag) = serde_json::from_str::<TagInfo>(&tag) {
                Some(tag.format())
            } else {
                None
            }
        })
    }))
}

fn create_tags_cache(args: &[&str], dir: &PathBuf) -> Result<(PathBuf, usize)> {
    let tags_stream = formatted_tags_stream(args, dir)?;
    let mut total = 0usize;
    let mut formatted_tags_stream = tags_stream.map(|x| {
        total += 1;
        x
    });
    let lines = formatted_tags_stream.join("\n");
    let cache = CacheEntry::create(args, Some(dir.clone()), total, lines)?;
    Ok((cache, total))
}

impl Tags {
    pub fn run(&self, no_cache: bool) -> Result<()> {
        let mut cmd_args = BASE_TAGS_ARGS.to_vec();
        let lang = if let Some(ref languages) = self.languages {
            format!("--languages={}", languages)
        } else {
            String::from("")
        };
        cmd_args.push(&lang);
        if self.forerunner {
            let (cache, total) = if no_cache {
                create_tags_cache(&cmd_args, &self.dir)?
            } else if let Ok(cached_info) = cache_exists(&cmd_args, &self.dir) {
                cached_info
            } else {
                create_tags_cache(&cmd_args, &self.dir)?
            };
            send_response_from_cache(&cache, total, SendResponse::Json, None);
            return Ok(());
        } else {
            crate::cmd::filter::dynamic::dyn_fuzzy_filter_and_rank(
                &self.query,
                Source::List(formatted_tags_stream(&cmd_args, &self.dir)?),
                None,
                Some(30),
                None,
                None,
                ContentFiltering::TagNameOnly,
            )?;
        }

        Ok(())
    }
}

#[test]
fn test_parse_ctags_line() {
    let data = r#"{"_type": "tag", "name": "Exec", "path": "crates/maple_cli/src/cmd/exec.rs", "pattern": "/^pub struct Exec {$/", "line": 10, "kind": "struct"}"#;
    let tag: TagInfo = serde_json::from_str(&data).unwrap();
    assert_eq!(tag.name, "Exec");
}
