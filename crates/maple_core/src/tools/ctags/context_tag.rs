use crate::tools::ctags::{BufferTag, CTAGS_BIN};
use rayon::prelude::*;
use std::io::Result;
use std::path::Path;
use std::process::Stdio;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use subprocess::Exec as SubprocessCommand;
use tokio::process::Command as TokioCommand;
use types::ClapItem;

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

fn subprocess_cmd(file: impl AsRef<std::ffi::OsStr>, has_json: bool) -> SubprocessCommand {
    if has_json {
        // Redirect stderr otherwise the warning message might occur `ctags: Warning: ignoring null tag...`
        SubprocessCommand::cmd("ctags")
            .stderr(subprocess::NullFile)
            .arg("--fields=+n")
            .arg("--output-format=json")
            .arg(file)
    } else {
        SubprocessCommand::cmd("ctags")
            .stderr(subprocess::NullFile)
            .arg("--fields=+Kn")
            .arg("-f")
            .arg("-")
            .arg(file)
    }
}

fn tokio_cmd(file: &Path, has_json: bool) -> TokioCommand {
    let mut tokio_cmd = TokioCommand::new("ctags");
    if has_json {
        tokio_cmd
            .stderr(Stdio::null())
            .arg("--fields=+n")
            .arg("--output-format=json")
            .arg(file);
    } else {
        tokio_cmd
            .stderr(Stdio::null())
            .arg("--fields=+Kn")
            .arg("-f")
            .arg("-")
            .arg(file);
    }

    tokio_cmd
}

fn find_context_tag(superset_tags: Vec<BufferTag>, at: usize) -> Option<BufferTag> {
    match superset_tags.binary_search_by_key(&at, |tag| tag.line_number) {
        Ok(_l) => None, // Skip if the line is exactly a tag line.
        Err(_l) => {
            let context_tags = superset_tags
                .into_par_iter()
                .filter(|tag| CONTEXT_KINDS.contains(&tag.kind.as_ref()))
                .collect::<Vec<_>>();

            match context_tags.binary_search_by_key(&at, |tag| tag.line_number) {
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
///
/// NOTE: I don't know why, but this may take forever to complete somehow, making the async runtime blocked.
pub async fn current_context_tag_async(file: &Path, at: usize) -> Option<BufferTag> {
    let superset_tags = if CTAGS_BIN.has_json_feature() {
        collect_superset_context_tags_async(tokio_cmd(file, true), BufferTag::from_json_line, at)
            .await
    } else {
        collect_superset_context_tags_async(tokio_cmd(file, false), BufferTag::from_raw_line, at)
            .await
    };

    find_context_tag(superset_tags.ok()?, at)
}

/// Returns the method/function context associated with line `at`.
pub fn current_context_tag(file: &Path, at: usize) -> Option<BufferTag> {
    let superset_tags = if CTAGS_BIN.has_json_feature() {
        collect_superset_context_tags(subprocess_cmd(file, true), BufferTag::from_json_line, at)
    } else {
        collect_superset_context_tags(subprocess_cmd(file, false), BufferTag::from_raw_line, at)
    };

    find_context_tag(superset_tags.ok()?, at)
}

pub fn buffer_tags_lines(
    file: impl AsRef<std::ffi::OsStr>,
    force_raw: bool,
) -> Result<Vec<String>> {
    let (tags, max_name_len) = if CTAGS_BIN.has_json_feature() && !force_raw {
        collect_buffer_tags(subprocess_cmd(file, true), BufferTag::from_json_line)?
    } else {
        collect_buffer_tags(subprocess_cmd(file, false), BufferTag::from_raw_line)?
    };

    Ok(tags
        .par_iter()
        .map(|s| s.format_buffer_tag(max_name_len))
        .collect::<Vec<_>>())
}

pub fn fetch_buffer_tags(file: impl AsRef<std::ffi::OsStr>) -> Result<Vec<BufferTag>> {
    let (mut tags, _max_name_len) = if CTAGS_BIN.has_json_feature() {
        collect_buffer_tags(subprocess_cmd(file, true), BufferTag::from_json_line)?
    } else {
        collect_buffer_tags(subprocess_cmd(file, false), BufferTag::from_raw_line)?
    };

    tags.par_sort_unstable_by_key(|x| x.line_number);

    Ok(tags)
}

pub fn buffer_tag_items(
    file: impl AsRef<std::ffi::OsStr>,
    force_raw: bool,
) -> Result<Vec<Arc<dyn ClapItem>>> {
    let (tags, max_name_len) = if CTAGS_BIN.has_json_feature() && !force_raw {
        collect_buffer_tags(subprocess_cmd(file, true), BufferTag::from_json_line)?
    } else {
        collect_buffer_tags(subprocess_cmd(file, false), BufferTag::from_raw_line)?
    };

    Ok(tags
        .into_par_iter()
        .map(|tag| Arc::new(tag.into_buffer_tag_item(max_name_len)) as Arc<dyn ClapItem>)
        .collect::<Vec<_>>())
}

fn collect_buffer_tags(
    cmd: SubprocessCommand,
    parse_tag: impl Fn(&str) -> Option<BufferTag> + Send + Sync,
) -> Result<(Vec<BufferTag>, usize)> {
    let max_name_len = AtomicUsize::new(0);

    let tags = crate::process::subprocess::exec(cmd)?
        .map_while(Result::ok)
        .par_bridge()
        .filter_map(|s| {
            let maybe_tag = parse_tag(&s);
            if let Some(ref tag) = maybe_tag {
                max_name_len.fetch_max(tag.name.len(), Ordering::SeqCst);
            }
            maybe_tag
        })
        .collect::<Vec<_>>();

    Ok((tags, max_name_len.into_inner()))
}

fn collect_superset_context_tags(
    cmd: SubprocessCommand,
    parse_tag: impl Fn(&str) -> Option<BufferTag> + Send + Sync,
    target_lnum: usize,
) -> Result<Vec<BufferTag>> {
    let mut tags = crate::process::subprocess::exec(cmd)?
        .map_while(Result::ok)
        .par_bridge()
        .filter_map(|s| parse_tag(&s))
        // the line of method/function name is lower.
        .filter(|tag| {
            tag.line_number <= target_lnum && CONTEXT_SUPERSET.contains(&tag.kind.as_ref())
        })
        .collect::<Vec<_>>();

    tags.par_sort_unstable_by_key(|x| x.line_number);

    Ok(tags)
}

async fn collect_superset_context_tags_async(
    cmd: TokioCommand,
    parse_tag: impl Fn(&str) -> Option<BufferTag> + Send + Sync,
    target_lnum: usize,
) -> Result<Vec<BufferTag>> {
    let mut cmd = cmd;

    let mut tags = cmd
        .output()
        .await?
        .stdout
        .par_split(|x| x == &b'\n')
        .filter_map(|s| parse_tag(&String::from_utf8_lossy(s)))
        // the line of method/function name is lower.
        .filter(|tag| {
            tag.line_number <= target_lnum && CONTEXT_SUPERSET.contains(&tag.kind.as_ref())
        })
        .collect::<Vec<_>>();

    tags.par_sort_unstable_by_key(|x| x.line_number);

    Ok(tags)
}
