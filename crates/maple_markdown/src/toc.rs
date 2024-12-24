use once_cell::sync::Lazy;
use percent_encoding::{percent_encode, CONTROLS};
use regex::Regex;
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
        Self {
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
            let title_link = strip_backticks(title);
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
                    slugify(&title_link)
                ))
            } else {
                Some(format!(
                    "{indent_before_bullet}{bullet}{indent_after_bullet}[{title}](#{})",
                    slugify(&title_link)
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
    Ok(utils::io::read_lines(input_file)?
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

pub fn generate_toc(
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

pub fn find_toc_range(input_file: impl AsRef<Path>) -> std::io::Result<Option<(usize, usize)>> {
    let mut start = 0;

    for (idx, line) in utils::io::read_lines(input_file)?
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

fn strip_backticks(input: &str) -> String {
    // Define a regex to match text enclosed in backticks
    static RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"`([^`]*)`").unwrap());

    // Replace the matched pattern, keeping the inner text unchanged
    RE.replace_all(input, "$1").to_string()
}

#[test]
fn test_heading() {
    let heading: Heading = "### run-`subcoin import-blocks`".parse().unwrap();
    assert_eq!(
        heading.title.clone(),
        "run-`subcoin import-blocks`".to_string()
    );
    assert_eq!(
        heading
            .format(&TocConfig {
                max_depth: Some(4),
                ..Default::default()
            })
            .unwrap(),
        "    *   [run-`subcoin import-blocks`](#run-subcoin-import-blocks)".to_string()
    );
}
