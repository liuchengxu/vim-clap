use crate::cmd::cache::{cache_exists, send_response_from_cache, CacheEntry, SendResponse};
use anyhow::Result;
use filter::{matcher::LineSplitter, Source};
use itertools::Itertools;
use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use structopt::StructOpt;
use utility::clap_cache_dir;

lazy_static! {
    static ref HOME: Option<PathBuf> = dirs::home_dir();
}

#[derive(Serialize, Deserialize, Debug)]
struct TagInfo {
    name: String,
    path: String,
    pattern: String,
    kind: String,
}

impl TagInfo {
    pub fn format(&self, cwd: &PathBuf, winwidth: usize) -> String {
        let name = format!("{} ", self.name);
        let taken_width = name.len() + 1;
        let path_len = self.path.len() + 2;
        let mut adjustment = 0;

        let mut home_path = PathBuf::new();
        let path = Path::new(&self.path);
        let path = path.strip_prefix(cwd).unwrap_or(
            // FIXME: is there a way to avoid cloning HOME?
            HOME.clone()
                .map(|home| {
                    path.strip_prefix(home)
                        .map(|path| {
                            home_path.push("~");
                            home_path = home_path.join(path);
                            home_path.as_path()
                        })
                        .unwrap_or(path)
                })
                .unwrap_or(path),
        );
        let path = path.display().to_string();

        let path_label = if taken_width > winwidth {
            format!("[{}]", path)
        } else {
            let available_width = winwidth - taken_width;
            if path_len > available_width && available_width > 3 {
                let diff = path_len - available_width;
                adjustment = 2;
                format!("[…{}]", path.chars().skip(diff + 2).collect::<String>())
            } else {
                format!("[{}]", path)
            }
        };

        let path_len = path_label.len();
        let text_width = if path_len < winwidth {
            winwidth - path_len
        } else {
            winwidth
        } + adjustment;

        format!(
            "{text:<text_width$}{path_label}:::{path}:::{pattern}",
            text = name,
            text_width = text_width,
            path_label = path_label,
            path = self.path,
            pattern = self.pattern,
        )
    }

    pub fn parse(base: &PathBuf, input: &str) -> Result<Self, std::string::ParseError> {
        let mut field = 0;
        let mut index = 0;
        let mut index_last = 0;

        let mut name = String::from("");
        let mut path = String::from("");
        let mut pattern = String::from("");
        let mut kind = String::from("");

        for c in input.chars() {
            if c == '\t' {
                match field {
                    0 => name = String::from(&input[index_last..index]),
                    1 => {
                        let path_buf = base.join(String::from(&input[index_last..index]));
                        path = path_buf.as_path().display().to_string();
                    }
                    2 => pattern = String::from(&input[index_last..index]),
                    3 => kind = String::from(&input[index_last..index]),
                    _ => {}
                }
                field += 1;
                index_last = index + c.len_utf8();
            }
            index += c.len_utf8();
        }

        // NOTE: we're not handling incorrectly formed tags because I don't feel
        // it's worth it. This might be revised if tagfile validation is someday
        // a concern.

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
}

fn read_tag_files<'a>(
    cwd: &'a PathBuf,
    winwidth: usize,
    files: &'a Vec<[PathBuf; 2]>,
) -> Result<impl Iterator<Item = String> + 'a> {
    let streams = files
        .into_iter()
        .map(move |path| read_tag_file(path, &cwd, winwidth))
        .flatten();

    let stream = Box::new(std::iter::empty()) as Box<dyn Iterator<Item = String>>;
    let stream = streams.fold(stream, |acc, f| {
        Box::new(acc.chain(f)) as Box<dyn Iterator<Item = String>>
    });

    Ok(stream)
}

fn read_tag_file<'a>(
    paths: &'a [PathBuf; 2],
    cwd: &'a PathBuf,
    winwidth: usize,
) -> Result<impl Iterator<Item = String> + 'a> {
    let file = File::open(&paths[0]).unwrap();

    Ok(BufReader::new(file).lines().filter_map(move |line| {
        line.ok().and_then(|input| {
            if input.starts_with("!_TAG") {
                None
            } else if let Ok(tag) = TagInfo::parse(&paths[1], &input) {
                Some(tag.format(&cwd, winwidth))
            } else {
                None
            }
        })
    }))
}

fn create_tags_cache(
    cwd: &PathBuf,
    winwidth: usize,
    args: &[&str],
    files: &Vec<[PathBuf; 2]>,
) -> Result<(PathBuf, usize)> {
    let tags_stream = read_tag_files(cwd, winwidth, files)?;
    let mut total = 0usize;
    let mut read_tag_files = tags_stream.map(|x| {
        total += 1;
        x
    });
    let lines = read_tag_files.join("\n");
    let cache = CacheEntry::create(args, None, total, lines)?;
    Ok((cache, total))
}

fn get_args_from_files(files: &Vec<PathBuf>) -> Vec<String> {
    files
        .iter()
        .map(|f| f.as_path().display().to_string())
        .collect::<Vec<_>>()
}

fn get_paths_from_files<'a>(files: &'a Vec<PathBuf>) -> Vec<[PathBuf; 2]> {
    files
        .iter()
        .map(|path| {
            let mut dirname = path.clone();
            dirname.pop();
            [path.to_owned(), dirname]
        })
        .collect::<Vec<_>>()
}

impl TagFiles {
    pub fn run(&self, options: &crate::Maple) -> Result<()> {
        // In case of passing an invalid icon-painter option.
        /* let icon_painter = options
         *     .icon_painter
         *     .clone()
         *     .map(|_| icon::IconPainter::ProjTags); */

        let cwd = options.cwd.clone().unwrap();
        let winwidth = options.winwidth.unwrap_or(120);
        let cache_dir = clap_cache_dir();

        let files = &self.files.clone();
        let tag_paths = get_paths_from_files(files);

        if self.forerunner {
            let arg_files = get_args_from_files(&files);
            let args = arg_files.iter().map(String::as_str).collect::<Vec<_>>();

            let (cache, total) = if options.no_cache {
                create_tags_cache(&cwd, winwidth, &args, &tag_paths)?
            } else if let Ok(cached_info) = cache_exists(&args, &cache_dir) {
                cached_info
            } else {
                create_tags_cache(&cwd, winwidth, &args, &tag_paths)?
            };
            send_response_from_cache(&cache, total, SendResponse::Json, None);
            return Ok(());
        } else {
            filter::dyn_run(
                &self.query,
                Source::List(read_tag_files(&cwd, winwidth, &tag_paths)?),
                None,
                Some(30),
                None,
                None,
                LineSplitter::TagNameOnly,
            )?;
        }

        Ok(())
    }
}
