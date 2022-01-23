pub mod recursive;
pub mod tagsfile;

use std::ops::Deref;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use anyhow::Result;
use filter::subprocess::Exec;
use itertools::Itertools;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use structopt::StructOpt;

use crate::app::Params;
use crate::paths::AbsPathBuf;
use crate::tools::ctags::{CTAGS_HAS_JSON_FEATURE, EXCLUDE};

/// Generate ctags recursively given the directory.
#[derive(StructOpt, Debug, Clone)]
pub struct SharedParams {
    /// The directory for executing the ctags command.
    #[structopt(long, parse(from_os_str))]
    dir: Option<PathBuf>,

    /// Specify the language.
    #[structopt(long)]
    languages: Option<String>,

    /// Exclude files and directories matching 'pattern'.
    ///
    /// Will be translated into ctags' option: --exclude=pattern.
    #[structopt(
        long,
        default_value = EXCLUDE,
        use_delimiter = true
    )]
    exclude: Vec<String>,

    /// Specify the input files.
    // - notify the tags update on demand.
    #[structopt(long)]
    files: Vec<AbsPathBuf>,
}

impl SharedParams {
    pub fn exclude_opt(&self) -> String {
        self.exclude
            .iter()
            .map(|x| format!("--exclude={}", x))
            .join(" ")
    }

    pub fn dir(&self) -> Result<PathBuf> {
        let dir = match self.dir {
            Some(ref d) => d.clone(),
            None => std::env::current_dir()?,
        };

        Ok(dir)
    }
}

/// Ctags command.
#[derive(StructOpt, Debug, Clone)]
pub enum Ctags {
    RecursiveTags(recursive::RecursiveTags),
    TagsFile(tagsfile::TagsFile),
    /// Prints the tags of an input file.
    BufferTags {
        #[structopt(long)]
        file: AbsPathBuf,
    },
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
struct BufferTagInfo {
    name: String,
    pattern: String,
    line: usize,
    kind: String,
}

impl BufferTagInfo {
    /// Returns the display line for BuiltinHandle, no icon attached.
    fn display(&self, max_name_len: usize) -> String {
        let pattern_len = self.pattern.len();

        let name_line = format!("{}:{}", self.name, self.line);

        let kind = format!("[{}]", self.kind);
        format!(
            "{name_group:<name_group_width$} {kind:<kind_width$} {pattern}",
            name_group = name_line,
            name_group_width = max_name_len + 6,
            kind = kind,
            kind_width = 20,
            pattern = self.pattern[2..pattern_len - 2].trim()
        )
    }
}

pub fn buffer_tags_lines(file: impl AsRef<std::ffi::OsStr>) -> Result<Vec<String>> {
    use std::io::BufRead;

    let stdout = Exec::cmd("ctags")
        .arg("--fields=+n")
        .arg("--output-format=json")
        .arg(file)
        .stream_stdout()?;

    let max_name_len = AtomicUsize::new(0);

    let tags = std::io::BufReader::with_capacity(8 * 1024 * 1024, stdout)
        .lines()
        .flatten()
        .par_bridge()
        .filter_map(|s| {
            let maybe_tag_info = serde_json::from_str::<BufferTagInfo>(&s).ok();
            if let Some(ref tag_info) = maybe_tag_info {
                max_name_len.fetch_max(tag_info.name.len(), Ordering::SeqCst);
            }
            maybe_tag_info
        })
        .collect::<Vec<_>>();

    let max_name_len = max_name_len.into_inner();

    Ok(tags
        .par_iter()
        .map(|s| s.display(max_name_len))
        .collect::<Vec<_>>())
}

impl Ctags {
    pub fn run(&self, params: Params) -> Result<()> {
        match self {
            Self::RecursiveTags(recursive_tags) => recursive_tags.run(params),
            Self::TagsFile(tags_file) => tags_file.run(params),
            Self::BufferTags { file } => {
                if *CTAGS_HAS_JSON_FEATURE.deref() {
                    let lines = buffer_tags_lines(file.to_string())?;
                    for line in lines {
                        println!("{}", line);
                    }
                } else {
                    Exec::cmd("ctags")
                        .arg("-n")
                        .arg("-f")
                        .arg("-")
                        .arg(file.to_string())
                        .stream_stdout()?;

                    /*
                    let lines = std::io::BufReader::with_capacity(8 * 1024 * 1024, stdout)
                        .lines()
                        .flatten()
                        .filter_map(|s| TagInfo::from_ctags(&s))
                        .collect::<Vec<_>>();
                    */
                }
                Ok(())
            }
        }
    }
}
