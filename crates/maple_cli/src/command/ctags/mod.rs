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
        /// Use the raw output format even json output is supported, for testing purpose.
        #[structopt(long)]
        force_raw: bool,

        #[structopt(long)]
        file: AbsPathBuf,
    },
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Default)]
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

    // The last scope field is optional.
    //
    // Blines	crates/maple_cli/src/app.rs	/^    Blines(command::blines::Blines),$/;"	enumerator	line:39	enum:Cmd
    fn from_ctags_raw(line: &str) -> Option<Self> {
        let mut items = line.split('\t');

        let name = items.next()?.into();
        let _path = items.next()?;

        let mut t = Self {
            name,
            ..Default::default()
        };

        let others = items.join("\t");

        if let Some((tagaddress, kind_line_scope)) = others.rsplit_once(";\"") {
            t.pattern = String::from(&tagaddress[2..]);

            let mut iter = kind_line_scope.split_whitespace();

            t.kind = iter.next()?.into();

            t.line = iter.next().and_then(|s| {
                s.split_once(':')
                    .and_then(|(_, line)| line.parse::<usize>().ok())
            })?;

            Some(t)
        } else {
            None
        }
    }
}

fn build_cmd_in_json_format(file: impl AsRef<std::ffi::OsStr>) -> Exec {
    Exec::cmd("ctags")
        .arg("--fields=+n")
        .arg("--output-format=json")
        .arg(file)
}

fn build_cmd_in_raw_format(file: impl AsRef<std::ffi::OsStr>) -> Exec {
    Exec::cmd("ctags")
        .arg("--fields=+Kn")
        .arg("-f")
        .arg("-")
        .arg(file)
}

pub fn buffer_tags_lines(file: impl AsRef<std::ffi::OsStr>) -> Result<Vec<String>> {
    if *CTAGS_HAS_JSON_FEATURE.deref() {
        let cmd = build_cmd_in_json_format(file);
        buffer_tags_lines_inner(cmd, |s: &str| serde_json::from_str::<BufferTagInfo>(s).ok())
    } else {
        let cmd = build_cmd_in_raw_format(file);
        buffer_tags_lines_inner(cmd, |s: &str| BufferTagInfo::from_ctags_raw(s))
    }
}

fn buffer_tags_lines_inner(
    cmd: Exec,
    parse_fn: impl Fn(&str) -> Option<BufferTagInfo> + Send + Sync,
) -> Result<Vec<String>> {
    use std::io::BufRead;

    let stdout = cmd.stream_stdout()?;

    let max_name_len = AtomicUsize::new(0);

    let tags = std::io::BufReader::with_capacity(8 * 1024 * 1024, stdout)
        .lines()
        .flatten()
        .par_bridge()
        .filter_map(|s| {
            let maybe_tag_info = parse_fn(&s);
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
            Self::BufferTags { file, force_raw } => {
                let lines = if *CTAGS_HAS_JSON_FEATURE.deref() && !force_raw {
                    let cmd = build_cmd_in_json_format(file.as_ref());
                    buffer_tags_lines_inner(cmd, |s: &str| {
                        serde_json::from_str::<BufferTagInfo>(s).ok()
                    })?
                } else {
                    let cmd = build_cmd_in_raw_format(file.as_ref());
                    buffer_tags_lines_inner(cmd, |s: &str| BufferTagInfo::from_ctags_raw(s))?
                };

                for line in lines {
                    println!("{}", line);
                }

                Ok(())
            }
        }
    }
}
