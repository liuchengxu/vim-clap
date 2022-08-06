mod context_tag;

pub use context_tag::{
    buffer_tag_items, buffer_tags_lines, current_context_tag, current_context_tag_async,
};

use anyhow::{Context, Result};
use clap::Parser;
use itertools::Itertools;
use matcher::{ClapItem, MatchScope};
use serde::{Deserialize, Serialize};
use types::FuzzyText;

use crate::app::Params;
use crate::paths::AbsPathBuf;

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

        let lines = buffer_tags_lines(self.file.as_ref(), self.force_raw)?;

        for line in lines {
            println!("{}", line);
        }

        Ok(())
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Default)]
pub struct BufferTagInfo {
    pub name: String,
    pub pattern: String,
    pub line: usize,
    pub kind: String,
}

#[derive(Debug)]
pub struct BufferTagItem {
    pub pattern: String,
    pub name: String,
    pub output_text: String,
}

impl ClapItem for BufferTagItem {
    fn raw_text(&self) -> &str {
        &self.output_text
    }

    fn match_text(&self) -> &str {
        self.raw_text()
    }

    fn fuzzy_text(&self, _match_scope: MatchScope) -> Option<FuzzyText> {
        Some(FuzzyText::new(&self.name, 0))
    }

    fn bonus_text(&self) -> &str {
        &self.pattern
    }
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

    pub fn into_buffer_tag_item(self, max_name_len: usize) -> BufferTagItem {
        let output_text = self.format_buffer_tags(max_name_len);
        BufferTagItem {
            pattern: self.pattern,
            name: self.name,
            output_text,
        }
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
