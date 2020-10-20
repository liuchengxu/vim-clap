use crate::cmd::cache::{cache_exists, send_response_from_cache, CacheEntry, SendResponse};
use anyhow::Result;
use filter::{matcher::LineSplitter, Source};
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{Read, BufRead, BufReader};
use std::path::PathBuf;
use std::str::FromStr;
use itertools::Itertools;
use structopt::StructOpt;
use utility::clap_cache_dir;

#[derive(Serialize, Deserialize, Debug)]
struct TagInfo {
    name: String,
    path: String,
    pattern: String,
    kind: String,
}

impl TagInfo {
    pub fn format(&self, winwidth: usize) -> String {
        let name = format!("{}", self.name);
        let path = format!("[{}]", self.path);
        let path_len = std::cmp::min(winwidth / 2, path.len());
        format!(
            "{text:<width1$} {path:<width2$}",
            text = name,
            width1 = winwidth - 4 - path_len,
            path = path,
            width2 = path_len,
        )
    }
}

impl FromStr for TagInfo {
    type Err = std::string::ParseError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        let mut field = 0;
        let mut index = 0;
        let mut index_last = 0;

        let mut name    = String::from("");
        let mut path    = String::from("");
        let mut pattern = String::from("");
        let mut kind    = String::from("");

        for c in input.chars() {
            if c == '\t' {
                match field {
                    0 => { name    = String::from(&input[index_last..index]) },
                    1 => { path    = String::from(&input[index_last..index]) },
                    2 => { pattern = String::from(&input[index_last..index]) },
                    3 => { kind    = String::from(&input[index_last..index]) },
                    _ => { },
                }
                field += 1;
                index_last = index + c.len_utf8();
            }

            index += c.len_utf8();
        }

        Ok(TagInfo {
            name,
            path,
            pattern,
            kind,
        })
    }
}


/// Generate ctags recursively given the directory.
#[derive(StructOpt, Debug, Clone)]
pub struct TagFiles {
    /// Initial query string
    #[structopt(index = 1, short, long)]
    query: String,

    /// The directory to generate recursive ctags.
    #[structopt(long, parse(from_os_str))]
    files: Vec<PathBuf>,

    /// Specify the language.
    #[structopt(long = "languages")]
    languages: Option<String>,

    /// Read input from a cached grep tempfile, only absolute file path is supported.
    #[structopt(long = "input", parse(from_os_str))]
    input: Option<PathBuf>,

    /// Runs as the forerunner job, create the new cache entry.
    #[structopt(short, long)]
    forerunner: bool,

    /// Exclude files and directories matching 'pattern'.
    ///
    /// Will be translated into ctags' option: --exclude=pattern.
    #[structopt(long, default_value = ".git,*.json,node_modules,target")]
    exclude: Vec<String>,
}

fn read_tag_files(winwidth: usize, files: &Vec<PathBuf>) -> Result<impl Iterator<Item = String>> {
    // let stream = concat_path(files.clone());

    // Transform list of paths into a list of files
    let files: Vec<File> = files.into_iter().map(|f| File::open(f)).flatten().collect();

    let bufreader = Box::new(std::io::empty()) as Box<dyn Read>;
    let stream =
        files
            .into_iter()
            .fold(bufreader, |acc, f| Box::new(acc.chain(f)) as Box<dyn Read>);

    Ok(BufReader::new(stream).lines().filter_map(move |line| {
        line.ok().and_then(|input| {
            if input.starts_with("!_TAG") {
                None
            } else if let Ok(tag) = TagInfo::from_str(&input) {
                Some(tag.format(winwidth))
            } else {
                None
            }
        })
    }))
}

fn create_tags_cache(winwidth: usize, args: &[&str], files: &Vec<PathBuf>) -> Result<(PathBuf, usize)> {
    let tags_stream = read_tag_files(winwidth, files)?;
    let mut total = 0usize;
    let mut read_tag_files = tags_stream.map(|x| {
        total += 1;
        x
    });
    let lines = read_tag_files.join("\n");
    let cache = CacheEntry::create(args, None, total, lines)?;
    Ok((cache, total))
}

impl TagFiles {
    pub fn run(
        &self,
        winwidth: Option<usize>,
        no_cache: bool,
        icon_painter: Option<icon::IconPainter>,
    ) -> Result<()> {
        // In case of passing an invalid icon-painter option.
        let icon_painter = icon_painter.map(|_| icon::IconPainter::ProjTags);

        // let mut cmd_args = BASE_TAGS_CMD
        //     .split_whitespace()
        //     .map(Into::into)
        //     .collect::<Vec<_>>();

        // let exclude = self
        //     .exclude
        //     .iter()
        //     .map(|x| x.split(',').collect::<Vec<_>>())
        //     .flatten()
        //     .map(|x| format!("--exclude={}", x))
        //     .collect::<Vec<_>>();

        // cmd_args.extend(exclude);

        // if let Some(ref languages) = self.languages {
        //     cmd_args.push(format!("--languages={}", languages));
        // };

        // let cmd_args = cmd_args.iter().map(|x| x.as_str()).collect::<Vec<_>>();

        let files =
            &self.files.iter()
                .map(|f| f.as_path().display().to_string())
                .collect::<Vec<_>>();
        let args =
            files.iter()
                .map(String::as_str)
                .collect::<Vec<_>>();
        let dir = clap_cache_dir();
        let winwidth = winwidth.unwrap_or(120);

        if self.forerunner {
            let (cache, total) = if no_cache {
                create_tags_cache(winwidth, &args, &self.files)?
            } else if let Ok(cached_info) = cache_exists(&args, &dir) {
                cached_info
            } else {
                create_tags_cache(winwidth, &args, &self.files)?
            };
            send_response_from_cache(&cache, total, SendResponse::Json, icon_painter);
            return Ok(());
        } else {
            filter::dyn_run(
                &self.query,
                Source::List(read_tag_files(winwidth, &self.files)?),
                None,
                Some(30),
                None,
                icon_painter,
                LineSplitter::TagNameOnly,
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
