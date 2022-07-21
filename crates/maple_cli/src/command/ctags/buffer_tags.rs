use std::ops::Deref;
use std::path::Path;
use std::process::Stdio;
use std::sync::atomic::{AtomicUsize, Ordering};

use anyhow::{Context, Result};
use clap::Parser;
use itertools::Itertools;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use subprocess::{Exec as SubprocessCommand, Redirection};
use tokio::process::Command as TokioCommand;

use crate::app::Params;
use crate::paths::AbsPathBuf;
use crate::tools::ctags::CTAGS_HAS_JSON_FEATURE;

/// Prints the tags for a specific file.
#[derive(Parser, Debug, Clone)]
pub struct BufferTags {
    /// Show the nearest function/method to a specific line.
    #[clap(long)]
    current_context: Option<usize>,

    /// Use the raw output format even json output is supported, for testing purpose.
    #[clap(long)]
    force_raw: bool,

    #[clap(long)]
    file: AbsPathBuf,
}

impl BufferTags {
    pub fn run(&self, _params: Params) -> Result<()> {
        if let Some(at) = self.current_context {
            let context_tag = current_context_tag(self.file.as_path(), at)
                .context("Error at finding the context tag info")?;
            println!("Context: {:?}", context_tag);
            return Ok(());
        }

        let lines = if *CTAGS_HAS_JSON_FEATURE.deref() && !self.force_raw {
            let cmd = subprocess_cmd_in_json_format(self.file.as_ref());
            buffer_tags_lines_inner(cmd, BufferTagInfo::from_ctags_json)?
        } else {
            let cmd = subprocess_cmd_in_json_format(self.file.as_ref());
            buffer_tags_lines_inner(cmd, BufferTagInfo::from_ctags_raw)?
        };

        for line in lines {
            println!("{}", line);
        }

        Ok(())
    }
}

const CONTEXT_KINDS: &[&str] = &[
    "function",
    "method",
    "module",
    "macro",
    "implementation",
    "interface",
];

const CONTEXT_SUPERSET: &[&str] = &[
    "function",
    "method",
    "module",
    "macro",
    "implementation",
    "interface",
    "struct",
    "field",
    "typedef",
    "enumerator",
];

fn subprocess_cmd_in_json_format(file: impl AsRef<std::ffi::OsStr>) -> SubprocessCommand {
    // Redirect stderr otherwise the warning message might occur `ctags: Warning: ignoring null tag...`
    SubprocessCommand::cmd("ctags")
        .stderr(Redirection::Merge)
        .arg("--fields=+n")
        .arg("--output-format=json")
        .arg(file)
}

fn subprocess_cmd_in_raw_format(file: impl AsRef<std::ffi::OsStr>) -> SubprocessCommand {
    // Redirect stderr otherwise the warning message might occur `ctags: Warning: ignoring null tag...`
    SubprocessCommand::cmd("ctags")
        .stderr(Redirection::Merge)
        .arg("--fields=+Kn")
        .arg("-f")
        .arg("-")
        .arg(file)
}

fn tokio_cmd_in_json_format(file: &Path) -> TokioCommand {
    let mut cmd = crate::process::tokio::build_command(format!(
        "ctags --fields=+n --output-format=json {}",
        file.display()
    ));
    cmd.stderr(Stdio::null());
    cmd
}

fn tokio_cmd_in_raw_format(file: &Path) -> TokioCommand {
    let mut cmd =
        crate::process::tokio::build_command(format!("ctags --fields=+Kn -f - {}", file.display()));
    cmd.stderr(Stdio::null());
    cmd
}

fn find_context_tag(superset_tags: Vec<BufferTagInfo>, at: usize) -> Option<BufferTagInfo> {
    match superset_tags.binary_search_by_key(&at, |tag| tag.line) {
        Ok(_l) => None, // Skip if the line is exactly a tag line.
        Err(_l) => {
            let context_tags = superset_tags
                .into_par_iter()
                .filter(|tag| CONTEXT_KINDS.contains(&tag.kind.as_ref()))
                .collect::<Vec<_>>();

            match context_tags.binary_search_by_key(&at, |tag| tag.line) {
                Ok(_) => None,
                Err(l) => {
                    let maybe_idx = l.checked_sub(1); // use the previous item.
                    maybe_idx.and_then(|idx| context_tags.into_iter().nth(idx))
                }
            }
        }
    }
}

/// Async version of [`current_context_tag`].
pub async fn current_context_tag_async(file: &Path, at: usize) -> Option<BufferTagInfo> {
    let superset_tags = if *CTAGS_HAS_JSON_FEATURE.deref() {
        let cmd = tokio_cmd_in_json_format(file);
        collect_superset_context_tags_async(cmd, BufferTagInfo::from_ctags_json, at)
            .await
            .ok()?
    } else {
        let cmd = tokio_cmd_in_raw_format(file);
        collect_superset_context_tags_async(cmd, BufferTagInfo::from_ctags_raw, at)
            .await
            .ok()?
    };

    find_context_tag(superset_tags, at)
}

/// Returns the method/function context associated with line `at`.
pub fn current_context_tag(file: &Path, at: usize) -> Option<BufferTagInfo> {
    let superset_tags = if *CTAGS_HAS_JSON_FEATURE.deref() {
        let cmd = subprocess_cmd_in_json_format(file);
        collect_superset_context_tags(cmd, BufferTagInfo::from_ctags_json, at).ok()?
    } else {
        let cmd = subprocess_cmd_in_raw_format(file);
        collect_superset_context_tags(cmd, BufferTagInfo::from_ctags_raw, at).ok()?
    };

    find_context_tag(superset_tags, at)
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Default)]
pub struct BufferTagInfo {
    pub name: String,
    pub pattern: String,
    pub line: usize,
    pub kind: String,
}

impl BufferTagInfo {
    /// Returns the display line for BuiltinHandle, no icon attached.
    fn format_buffer_tags(&self, max_name_len: usize) -> String {
        let name_line = format!("{}:{}", self.name, self.line);

        let kind = format!("[{}]", self.kind);
        format!(
            "{name_group:<name_group_width$} {kind:<kind_width$} {pattern}",
            name_group = name_line,
            name_group_width = max_name_len + 6,
            kind = kind,
            kind_width = 10,
            pattern = self.extract_pattern().trim()
        )
    }

    #[inline]
    fn from_ctags_json(line: &str) -> Option<Self> {
        serde_json::from_str::<Self>(line).ok()
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

    pub fn extract_pattern(&self) -> &str {
        let pattern_len = self.pattern.len();
        &self.pattern[2..pattern_len - 2]
    }
}

pub fn buffer_tags_lines(file: impl AsRef<std::ffi::OsStr>) -> Result<Vec<String>> {
    if *CTAGS_HAS_JSON_FEATURE.deref() {
        let cmd = subprocess_cmd_in_json_format(file);
        buffer_tags_lines_inner(cmd, BufferTagInfo::from_ctags_json)
    } else {
        let cmd = subprocess_cmd_in_raw_format(file);
        buffer_tags_lines_inner(cmd, BufferTagInfo::from_ctags_raw)
    }
}

fn buffer_tags_lines_inner(
    cmd: SubprocessCommand,
    parse_fn: impl Fn(&str) -> Option<BufferTagInfo> + Send + Sync,
) -> Result<Vec<String>> {
    let max_name_len = AtomicUsize::new(0);

    let tags = crate::utils::lines(cmd)?
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
        .map(|s| s.format_buffer_tags(max_name_len))
        .collect::<Vec<_>>())
}

fn collect_superset_context_tags(
    cmd: SubprocessCommand,
    parse_fn: impl Fn(&str) -> Option<BufferTagInfo> + Send + Sync,
    target_lnum: usize,
) -> Result<Vec<BufferTagInfo>> {
    let mut tags = crate::utils::lines(cmd)?
        .flatten()
        .par_bridge()
        .filter_map(|s| parse_fn(&s))
        // the line of method/function name is lower.
        .filter(|tag| tag.line <= target_lnum && CONTEXT_SUPERSET.contains(&tag.kind.as_ref()))
        .collect::<Vec<_>>();

    tags.par_sort_unstable_by_key(|x| x.line);

    Ok(tags)
}

async fn collect_superset_context_tags_async(
    cmd: TokioCommand,
    parse_fn: impl Fn(&str) -> Option<BufferTagInfo> + Send + Sync,
    target_lnum: usize,
) -> Result<Vec<BufferTagInfo>> {
    let mut cmd = cmd;

    let mut tags = cmd
        .output()
        .await?
        .stdout
        .par_split(|x| x == &b'\n')
        .filter_map(|s| parse_fn(&String::from_utf8_lossy(s)))
        // the line of method/function name is lower.
        .filter(|tag| tag.line <= target_lnum && CONTEXT_SUPERSET.contains(&tag.kind.as_ref()))
        .collect::<Vec<_>>();

    tags.par_sort_unstable_by_key(|x| x.line);

    Ok(tags)
}
