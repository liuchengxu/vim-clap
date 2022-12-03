use crate::paths::AbsPathBuf;
use anyhow::Result;
use clap::Parser;
use std::borrow::Cow;
use std::collections::HashMap;
use std::io::Write;
use utility::read_lines;

/// Parse and display Vim helptags.
#[derive(Parser, Debug, Clone)]
pub struct Helptags {
    /// Tempfile containing the info of vim helptags.
    #[clap(index = 1, long)]
    meta_info: AbsPathBuf,
}

#[inline]
fn strip_trailing_slash(x: &str) -> Cow<str> {
    if x.ends_with('/') {
        let mut x: String = x.into();
        x.pop();
        x.into()
    } else {
        x.into()
    }
}

impl Helptags {
    pub fn run(self) -> Result<()> {
        let mut lines = read_lines(self.meta_info.as_ref())?;
        // line 1:/doc/tags,/doc/tags-cn
        // line 2:&runtimepath
        if let Some(Ok(doc_tags)) = lines.next() {
            if let Some(Ok(runtimepath)) = lines.next() {
                let lines =
                    generate_tag_lines(doc_tags.split(',').map(|s| s.to_string()), &runtimepath);
                let stdout = std::io::stdout();
                let mut lock = stdout.lock();
                for line in lines {
                    writeln!(lock, "{line}")?;
                }
            }
        }
        Ok(())
    }
}

pub fn generate_tag_lines(
    doc_tags: impl Iterator<Item = String>,
    runtimepath: &str,
) -> Vec<String> {
    let mut lines = Vec::new();
    for doc_tag in doc_tags {
        let tags_files = runtimepath
            .split(',')
            .map(|x| format!("{}{}", strip_trailing_slash(x), doc_tag));
        let mut seen = HashMap::new();
        let mut v: Vec<String> = Vec::new();
        for tags_file in tags_files {
            if let Ok(lines) = read_lines(tags_file) {
                lines.for_each(|line| {
                    if let Ok(helptag) = line {
                        v = helptag.split('\t').map(Into::into).collect();
                        if !seen.contains_key(&v[0]) {
                            seen.insert(v[0].clone(), format!("{:<60}\t{}", v[0], v[1]));
                        }
                    }
                });
            }
        }
        let mut tag_lines = seen.into_values().collect::<Vec<String>>();
        tag_lines.sort();

        lines.extend(tag_lines);
    }

    lines
}
