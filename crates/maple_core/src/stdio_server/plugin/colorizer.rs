use crate::stdio_server::input::{AutocmdEvent, AutocmdEventType, PluginAction};
use crate::stdio_server::plugin::{ClapPlugin, Toggle};
use crate::stdio_server::vim::Vim;
use anyhow::Result;
use once_cell::sync::Lazy;
use regex::Regex;
use std::collections::BTreeMap;
use std::path::Path;

static HEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"#([a-fA-F0-9]{3}|[a-fA-F0-9]{6})\b"#).unwrap());

#[derive(Debug, Clone, maple_derive::ClapPlugin)]
#[clap_plugin(id = "colorizer", actions = ["toggle"])]
pub struct ColorizerPlugin {
    vim: Vim,
    toggle: Toggle,
}

impl ColorizerPlugin {
    pub fn new(vim: Vim) -> Self {
        Self {
            vim,
            toggle: Toggle::Off,
        }
    }
}

#[derive(Debug, serde::Serialize)]
struct HighlightGroup {
    name: String,
    guibg: String,
    ctermbg: String,
}

#[derive(Debug, serde::Serialize)]
struct ColorInfo {
    col: usize,
    length: usize,
    highlight_group: HighlightGroup,
}

fn find_colors(input_file: impl AsRef<Path>) -> std::io::Result<BTreeMap<usize, Vec<ColorInfo>>> {
    let mut p: BTreeMap<usize, Vec<_>> = BTreeMap::new();

    for (index, line) in utils::read_lines(input_file)?
        .map_while(Result::ok)
        .enumerate()
    {
        for caps in HEX.captures_iter(&line) {
            if let Some(cap) = caps.get(0) {
                let hex_code = cap.as_str().to_string();

                let group_name: String = format!("ClapColorizer_{}", &hex_code[1..]);

                // 0-based
                let line_number = index;

                let color_info = ColorInfo {
                    col: cap.range().start,
                    length: cap.range().len(),
                    highlight_group: HighlightGroup {
                        name: group_name,
                        guibg: hex_code,
                        ctermbg: "0".to_string(),
                    },
                };

                if let Some(v) = p.get_mut(&line_number) {
                    v.push(color_info)
                } else {
                    p.insert(line_number, vec![color_info]);
                }
            }
        }
    }

    Ok(p)
}

#[async_trait::async_trait]
impl ClapPlugin for ColorizerPlugin {
    async fn handle_action(&mut self, action: PluginAction) -> Result<()> {
        let PluginAction { method, params: _ } = action;

        match method.as_str() {
            Self::TOGGLE => {
                let bufnr = self.vim.bufnr("").await?;

                if self.toggle.is_off() {
                    let file = self.vim.bufabspath(bufnr).await?;
                    let colors = find_colors(file)?;
                    if !colors.is_empty() {
                        self.vim
                            .exec("clap#plugin#colorizer#add_highlights", (bufnr, colors))?;
                    }
                } else {
                    self.vim
                        .exec("clap#plugin#colorizer#clear_highlights", bufnr)?;
                }

                self.toggle.switch();
            }
            _ => {}
        }

        Ok(())
    }

    async fn handle_autocmd(&mut self, _autocmd: AutocmdEvent) -> Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_color_patterns() {
        let line = r#"
      \ 16 : '#000000',  17 : '#00005f',  18 : '#000087',  19 : '#0000af',  20 : '#0000d7',  21 : '#0000ff',
"#;
        for caps in HEX.captures_iter(line) {
            if let Some(cap) = caps.get(0) {
                println!("Found color code: {}, {:?}", cap.as_str(), cap.range());
            }
        }
    }
}
