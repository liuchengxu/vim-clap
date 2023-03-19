use once_cell::sync::Lazy;
use percent_encoding::{percent_encode, CONTROLS};
use regex::Regex;
use std::path::Path;
use std::str::FromStr;

fn slugify(text: &str) -> String {
    percent_encode(text.replace(" ", "-").to_lowercase().as_bytes(), CONTROLS).to_string()
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
            bullet: String::from("* "),
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
        if trimmed.starts_with("#") {
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
            let indent = " ".repeat(config.indent).repeat(depth - config.min_depth);
            let bullet = &config.bullet;

            if config.no_link {
                Some(format!("{indent}{bullet} {title}"))
            } else {
                if let Some(cap) = MARKDOWN_LINK.captures(title) {
                    let title = cap.get(1).map(|x| x.as_str())?;
                    Some(format!("{indent}{bullet} [{title}]({title})"))
                } else {
                    Some(format!("{indent}{bullet} [{title}](#{})", slugify(title)))
                }
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
        .into_iter()
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

pub fn generate_toc(input_file: &Path, line_start: usize) -> std::io::Result<Vec<String>> {
    let toc_config = TocConfig::default();
    let mut toc = parse_toc(input_file, &toc_config, line_start)?;

    toc.insert(0, "<!-- clap-markdown-toc -->".to_string());
    toc.insert(1, Default::default());
    toc.push(Default::default());
    toc.push("<!-- /clap-markdown-toc -->".to_string());

    Ok(toc)
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
        println!("");
        for line in generate_toc(&file, 0).unwrap() {
            println!("{line}");
        }
    }
}
