#![allow(clippy::enum_variant_names)]

use crate::stdio_server::input::{AutocmdEvent, AutocmdEventType, PluginAction};
use crate::stdio_server::plugin::{ClapPlugin, PluginError, Toggle};
use crate::stdio_server::vim::Vim;
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

#[derive(Debug, Clone, maple_derive::ClapPlugin)]
#[clap_plugin(id = "markdown", actions = ["generateToc", "updateToc", "deleteToc"])]
pub struct Markdown {
    vim: Vim,
    toggle: Toggle,
}

impl Markdown {
    pub fn new(vim: Vim) -> Self {
        Self {
            vim,
            toggle: Toggle::Off,
        }
    }

    async fn update_toc(&self, bufnr: usize) -> Result<(), PluginError> {
        let file = self.vim.bufabspath(bufnr).await?;
        if let Some((start, end)) = find_toc_range(&file)? {
            let shiftwidth = self.vim.getbufvar("", "&shiftwidth").await?;
            // TODO: skip update if the new doc is the same as the old one.
            let new_toc = generate_toc(file, start + 1, shiftwidth)?;
            self.vim.deletebufline(bufnr, start + 1, end + 1).await?;
            self.vim.exec("append_and_write", json!([start, new_toc]))?;
        }
        Ok(())
    }
}

#[async_trait::async_trait]
impl ClapPlugin for Markdown {
    fn subscriptions(&self) -> &[AutocmdEventType] {
        &[]
    }

    async fn handle_autocmd(&mut self, _autocmd: AutocmdEvent) -> Result<(), PluginError> {
        if self.toggle.is_off() {
            return Ok(());
        }

        Ok(())
    }

    async fn handle_action(&mut self, action: PluginAction) -> Result<(), PluginError> {
        let PluginAction { method, params: _ } = action;
        match self.parse_action(method)? {
            MarkdownAction::GenerateToc => {
                let file = self.vim.current_buffer_path().await?;
                let curlnum = self.vim.line(".").await?;
                let shiftwidth = self.vim.getbufvar("", "&shiftwidth").await?;
                let mut toc = generate_toc(file, curlnum, shiftwidth)?;
                let prev_line = self.vim.curbufline(curlnum - 1).await?;
                if !prev_line.map(|line| line.is_empty()).unwrap_or(false) {
                    toc.push_front(Default::default());
                }
                self.vim
                    .exec("append_and_write", json!([curlnum - 1, toc]))?;
            }
            MarkdownAction::UpdateToc => {
                let bufnr = self.vim.bufnr("").await?;
                self.update_toc(bufnr).await?;
            }
            MarkdownAction::DeleteToc => {
                let file = self.vim.current_buffer_path().await?;
                let bufnr = self.vim.bufnr("").await?;
                if let Some((start, end)) = find_toc_range(file)? {
                    self.vim.deletebufline(bufnr, start + 1, end + 1).await?;
                }
            }
        }

        Ok(())
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
        for line in generate_toc(file, 0, 2).unwrap() {
            println!("{line}");
        }
    }
}
