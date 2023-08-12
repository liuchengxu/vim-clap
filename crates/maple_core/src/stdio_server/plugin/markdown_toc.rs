use crate::stdio_server::input::{PluginAction, PluginEvent};
use crate::stdio_server::plugin::{ActionType, ClapPlugin, PluginId};
use crate::stdio_server::vim::Vim;
use anyhow::{anyhow, Result};
use once_cell::sync::Lazy;
use percent_encoding::{percent_encode, CONTROLS};
use regex::Regex;
use serde_json::json;
use std::collections::VecDeque;
use std::path::Path;
use std::str::FromStr;

fn slugify(text: &str) -> String {
    percent_encode(text.replace(' ', "-").to_lowercase().as_bytes(), CONTROLS).to_string()
}

#[derive(Debug)]
pub struct TocConfig {
    pub bullet: String,
    pub indent: usize,
    pub max_depth: Option<usize>,
    pub min_depth: usize,
    pub header: Option<String>,
    pub no_link: bool,
}

impl Default for TocConfig {
    fn default() -> Self {
        TocConfig {
            bullet: String::from("*"),
            indent: 4,
            max_depth: None,
            min_depth: 1,
            no_link: false,
            header: Some(String::from("## Table of Contents")),
        }
    }
}

#[derive(Debug)]
pub struct Heading {
    pub depth: usize,
    pub title: String,
}

impl FromStr for Heading {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let trimmed = s.trim_end();
        if trimmed.starts_with('#') {
            let mut depth = 0usize;
            let title = trimmed
                .chars()
                .skip_while(|c| {
                    if *c == '#' {
                        depth += 1;
                        true
                    } else {
                        false
                    }
                })
                .collect::<String>()
                .trim_start()
                .to_owned();
            Ok(Heading {
                depth: depth - 1,
                title,
            })
        } else {
            Err(())
        }
    }
}

static MARKDOWN_LINK: Lazy<Regex> = Lazy::new(|| Regex::new(r"^\[(.*)\](.*)").unwrap());

impl Heading {
    fn format(&self, config: &TocConfig) -> Option<String> {
        if self.depth >= config.min_depth
            && config.max_depth.map(|d| self.depth <= d).unwrap_or(true)
        {
            let Self { depth, title } = self;
            let indent_before_bullet = " "
                .repeat(config.indent)
                .repeat(depth.saturating_sub(config.min_depth));
            let bullet = &config.bullet;
            let indent_after_bullet = " ".repeat(config.indent.saturating_sub(1));

            if config.no_link {
                Some(format!(
                    "{indent_before_bullet}{bullet}{indent_after_bullet}{title}"
                ))
            } else if let Some(cap) = MARKDOWN_LINK.captures(title) {
                let title = cap.get(1).map(|x| x.as_str())?;
                Some(format!(
                    "{indent_before_bullet}{bullet}{indent_after_bullet}[{title}](#{})",
                    slugify(title)
                ))
            } else {
                Some(format!(
                    "{indent_before_bullet}{bullet}{indent_after_bullet}[{title}](#{})",
                    slugify(title)
                ))
            }
        } else {
            None
        }
    }
}

enum CodeBlockStart {
    Backticks,
    Tides,
}

fn parse_toc(
    input_file: &Path,
    toc_config: &TocConfig,
    line_start: usize,
) -> std::io::Result<Vec<String>> {
    let mut code_fence = None;
    Ok(utils::read_lines(input_file)?
        .skip(line_start)
        .filter_map(Result::ok)
        .filter(|line| match &code_fence {
            None => {
                if line.starts_with("```") {
                    code_fence.replace(CodeBlockStart::Backticks);
                    false
                } else if line.starts_with("~~~") {
                    code_fence.replace(CodeBlockStart::Tides);
                    false
                } else {
                    true
                }
            }
            Some(code_block_start) => {
                match code_block_start {
                    CodeBlockStart::Backticks if line.starts_with("```") => {
                        code_fence.take();
                    }
                    CodeBlockStart::Tides if line.starts_with("~~~") => {
                        code_fence.take();
                    }
                    _ => {}
                }
                false
            }
        })
        .filter_map(|line| {
            line.parse::<Heading>()
                .ok()
                .and_then(|heading| heading.format(toc_config))
        })
        .collect())
}

fn generate_toc(
    input_file: impl AsRef<Path>,
    line_start: usize,
    shiftwidth: usize,
) -> std::io::Result<VecDeque<String>> {
    let toc_config = TocConfig {
        indent: shiftwidth,
        ..Default::default()
    };
    let toc = parse_toc(input_file.as_ref(), &toc_config, line_start)?;

    let mut full_toc = Vec::with_capacity(toc.len() + 4);
    full_toc.push("<!-- clap-markdown-toc -->".to_string());
    full_toc.push(Default::default());
    full_toc.extend(toc);
    full_toc.push(Default::default());
    full_toc.push("<!-- /clap-markdown-toc -->".to_string());

    Ok(full_toc.into())
}

fn find_toc_range(input_file: impl AsRef<Path>) -> std::io::Result<Option<(usize, usize)>> {
    let mut start = 0;

    for (idx, line) in utils::read_lines(input_file)?
        .map_while(Result::ok)
        .enumerate()
    {
        let line = line.trim();
        if line == "<!-- clap-markdown-toc -->" {
            start = idx;
        } else if line == "<!-- /clap-markdown-toc -->" {
            return Ok(Some((start, idx)));
        } else {
            continue;
        }
    }

    Ok(None)
}

#[derive(Debug, Clone)]
pub struct MarkdownPlugin {
    vim: Vim,
}

impl MarkdownPlugin {
    const GENERATE_TOC: &'static str = "markdown/generate-toc";
    const UPDATE_TOC: &'static str = "markdown/update-toc";
    const DELETE_TOC: &'static str = "markdown/delete-toc";

    pub const ID: PluginId = PluginId::Markdown;
    pub const ACTIONS: &[&'static str] = &[Self::GENERATE_TOC, Self::UPDATE_TOC, Self::DELETE_TOC];

    pub fn new(vim: Vim) -> Self {
        Self { vim }
    }
}

#[async_trait::async_trait]
impl ClapPlugin for MarkdownPlugin {
    fn id(&self) -> PluginId {
        Self::ID
    }

    fn actions(&self, action_type: ActionType) -> &[&'static str] {
        match action_type {
            ActionType::Callable => Self::ACTIONS,
            ActionType::All => Self::ACTIONS,
        }
    }

    async fn on_plugin_event(&mut self, plugin_event: PluginEvent) -> Result<()> {
        match plugin_event {
            PluginEvent::Autocmd(_) => Ok(()),
            PluginEvent::Action(plugin_action) => {
                let PluginAction { action, params: _ } = plugin_action;
                match action.as_str() {
                    Self::GENERATE_TOC => {
                        let curlnum = self.vim.line(".").await?;
                        let file = self.vim.current_buffer_path().await?;
                        let shiftwidth = self.vim.getbufvar("", "&shiftwidth").await?;
                        let mut toc = generate_toc(file, curlnum, shiftwidth)?;
                        let prev_line = self.vim.curbufline(curlnum - 1).await?;
                        if !prev_line.map(|line| line.is_empty()).unwrap_or(false) {
                            toc.push_front(Default::default());
                        }
                        self.vim
                            .exec("append_and_write", json!([curlnum - 1, toc]))?;
                    }
                    Self::UPDATE_TOC => {
                        let file = self.vim.current_buffer_path().await?;
                        let bufnr = self.vim.bufnr("").await?;
                        if let Some((start, end)) = find_toc_range(&file)? {
                            let shiftwidth = self.vim.getbufvar("", "&shiftwidth").await?;
                            // TODO: skip update if the new doc is the same as the old one.
                            let new_toc = generate_toc(file, start + 1, shiftwidth)?;
                            self.vim.deletebufline(bufnr, start + 1, end + 1).await?;
                            self.vim.exec("append_and_write", json!([start, new_toc]))?;
                        }
                    }
                    Self::DELETE_TOC => {
                        let file = self.vim.current_buffer_path().await?;
                        let bufnr = self.vim.bufnr("").await?;
                        if let Some((start, end)) = find_toc_range(file)? {
                            self.vim.deletebufline(bufnr, start + 1, end + 1).await?;
                        }
                    }
                    unknown_action => return Err(anyhow!("Unknown action: {unknown_action:?}")),
                }

                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_markdown_toc() {
        let file = std::env::current_dir()
            .unwrap()
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("README.md");
        println!();
        for line in generate_toc(&file, 0, 2).unwrap() {
            println!("{line}");
        }
    }
}
