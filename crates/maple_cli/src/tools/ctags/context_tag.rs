use std::ops::Deref;
use std::path::Path;
use std::process::Stdio;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use anyhow::Result;
use rayon::prelude::*;
use subprocess::{Exec as SubprocessCommand, Redirection};
use tokio::process::Command as TokioCommand;

use types::ClapItem;

use super::BufferTag;
use crate::tools::ctags::CTAGS_HAS_JSON_FEATURE;

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
        .stderr(Redirection::None)
        .arg("--fields=+n")
        .arg("--output-format=json")
        .arg(file)
}

fn subprocess_cmd_in_raw_format(file: impl AsRef<std::ffi::OsStr>) -> SubprocessCommand {
    // Redirect stderr otherwise the warning message might occur `ctags: Warning: ignoring null tag...`
    SubprocessCommand::cmd("ctags")
        .stderr(Redirection::None)
        .arg("--fields=+Kn")
        .arg("-f")
        .arg("-")
        .arg(file)
}

fn tokio_cmd_in_json_format(file: &Path) -> TokioCommand {
    let mut tokio_cmd = TokioCommand::new("ctags");
    tokio_cmd
        .stderr(Stdio::null())
        .arg("--fields=+n")
        .arg("--output-format=json")
        .arg(file);
    tokio_cmd
}

fn tokio_cmd_in_raw_format(file: &Path) -> TokioCommand {
    let mut tokio_cmd = TokioCommand::new("ctags");
    tokio_cmd
        .stderr(Stdio::null())
        .arg("--fields=+Kn")
        .arg("-f")
        .arg("-")
        .arg(file);
    tokio_cmd
}

fn find_context_tag(superset_tags: Vec<BufferTag>, at: usize) -> Option<BufferTag> {
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
pub async fn current_context_tag_async(file: &Path, at: usize) -> Option<BufferTag> {
    let superset_tags = if *CTAGS_HAS_JSON_FEATURE.deref() {
        let cmd = tokio_cmd_in_json_format(file);
        collect_superset_context_tags_async(cmd, BufferTag::from_ctags_json, at)
            .await
            .ok()?
    } else {
        let cmd = tokio_cmd_in_raw_format(file);
        collect_superset_context_tags_async(cmd, BufferTag::from_ctags_raw, at)
            .await
            .ok()?
    };

    find_context_tag(superset_tags, at)
}

/// Returns the method/function context associated with line `at`.
pub fn current_context_tag(file: &Path, at: usize) -> Option<BufferTag> {
    let superset_tags = if *CTAGS_HAS_JSON_FEATURE.deref() {
        let cmd = subprocess_cmd_in_json_format(file);
        collect_superset_context_tags(cmd, BufferTag::from_ctags_json, at).ok()?
    } else {
        let cmd = subprocess_cmd_in_raw_format(file);
        collect_superset_context_tags(cmd, BufferTag::from_ctags_raw, at).ok()?
    };

    find_context_tag(superset_tags, at)
}

pub fn buffer_tags_lines(
    file: impl AsRef<std::ffi::OsStr>,
    force_raw: bool,
) -> Result<Vec<String>> {
    if *CTAGS_HAS_JSON_FEATURE.deref() && !force_raw {
        let cmd = subprocess_cmd_in_json_format(file);
        buffer_tags_lines_inner(cmd, BufferTag::from_ctags_json)
    } else {
        let cmd = subprocess_cmd_in_raw_format(file);
        buffer_tags_lines_inner(cmd, BufferTag::from_ctags_raw)
    }
}

pub fn buffer_tag_items(
    file: impl AsRef<std::ffi::OsStr>,
    force_raw: bool,
) -> Result<Vec<Arc<dyn ClapItem>>> {
    let (tags, max_name_len) = if *CTAGS_HAS_JSON_FEATURE.deref() && !force_raw {
        let cmd = subprocess_cmd_in_json_format(file);
        collect_buffer_tag_info(cmd, BufferTag::from_ctags_json)?
    } else {
        let cmd = subprocess_cmd_in_raw_format(file);
        collect_buffer_tag_info(cmd, BufferTag::from_ctags_raw)?
    };

    Ok(tags
        .into_par_iter()
        .map(|tag_info| Arc::new(tag_info.into_buffer_tag_item(max_name_len)) as Arc<dyn ClapItem>)
        .collect::<Vec<_>>())
}

fn collect_buffer_tag_info(
    cmd: SubprocessCommand,
    parse_fn: impl Fn(&str) -> Option<BufferTag> + Send + Sync,
) -> Result<(Vec<BufferTag>, usize)> {
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

    Ok((tags, max_name_len.into_inner()))
}

fn buffer_tags_lines_inner(
    cmd: SubprocessCommand,
    parse_fn: impl Fn(&str) -> Option<BufferTag> + Send + Sync,
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
        .map(|s| s.format_buffer_tag(max_name_len))
        .collect::<Vec<_>>())
}

fn collect_superset_context_tags(
    cmd: SubprocessCommand,
    parse_fn: impl Fn(&str) -> Option<BufferTag> + Send + Sync,
    target_lnum: usize,
) -> Result<Vec<BufferTag>> {
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
    parse_fn: impl Fn(&str) -> Option<BufferTag> + Send + Sync,
    target_lnum: usize,
) -> Result<Vec<BufferTag>> {
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
