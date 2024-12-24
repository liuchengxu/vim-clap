use crate::stdio_server::input::PluginAction;
use crate::stdio_server::plugin::{ClapPlugin, PluginError, Toggle};
use crate::stdio_server::vim::Vim;
use colors_transform::{AlphaColor, Color, Hsl, Rgb};
use once_cell::sync::Lazy;
use regex::Regex;
use rgb2ansi256::rgb_to_ansi256;
use std::collections::BTreeMap;
use std::path::Path;

static HEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"#([a-fA-F0-9]{3}|[a-fA-F0-9]{6})\b").unwrap());

static RGB: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"rgb\s*\(\s*(\d{1,3})\s*,\s*(\d{1,3})\s*,\s*(\d{1,3})\s*\)").unwrap());

static RGB_ALPHA: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"rgba\s*\(\s*(\d{1,3})\s*,\s*(\d{1,3})\s*,\s*(\d{1,3})\s*,\s*([01]\.?\d*)\)")
        .unwrap()
});

static HSL: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"hsl\s*\(\s*(\d{1,3}\.?\d*)\s*,\s*(\d{1,3}\.?\d*)%\s*,\s*(\d{1,3}\.?\d*)%\)")
        .unwrap()
});

static HSL_ALPHA: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"hsla\s*\(\s*(\d{1,3}\.?\d*)\s*,\s*(\d{1,3}\.?\d*)%\s*,\s*(\d{1,3}\.?\d*)%,\s*([01]\.?\d*)\)")
        .unwrap()
});

#[derive(Debug, Clone, maple_derive::ClapPlugin)]
#[clap_plugin(id = "colorizer", actions = ["off", "toggle"])]
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
    ctermbg: u8,
}

#[derive(Debug, serde::Serialize)]
struct ColorInfo {
    col: usize,
    length: usize,
    highlight_group: HighlightGroup,
}

enum HexOrRgb {
    Hex(String),
    Rgb(Rgb),
}

fn find_colors(input_file: impl AsRef<Path>) -> std::io::Result<BTreeMap<usize, Vec<ColorInfo>>> {
    let mut p: BTreeMap<usize, Vec<_>> = BTreeMap::new();

    let mut insert_color_info = |line_number, m: regex::Match, color: HexOrRgb| {
        let (ctermbg, hex_code) = match color {
            HexOrRgb::Hex(hex_code) => {
                let Ok(ctermbg) = Rgb::from_hex_str(&hex_code).map(|rgb| {
                    let (r, g, b) = rgb.as_tuple();
                    rgb_to_ansi256(r as u8, g as u8, b as u8)
                }) else {
                    return;
                };

                (ctermbg, hex_code)
            }
            HexOrRgb::Rgb(rgb) => {
                let (r, g, b) = rgb.as_tuple();
                let ctermbg = rgb_to_ansi256(r as u8, g as u8, b as u8);
                (ctermbg, rgb.to_css_hex_string())
            }
        };

        let group_name: String = format!("ClapColorizer_{}", &hex_code[1..]);

        let color_info = ColorInfo {
            col: m.range().start,
            length: m.range().len(),
            highlight_group: HighlightGroup {
                name: group_name,
                guibg: hex_code,
                ctermbg,
            },
        };

        if let Some(v) = p.get_mut(&line_number) {
            v.push(color_info)
        } else {
            p.insert(line_number, vec![color_info]);
        }
    };

    // 0-based line_number
    for (line_number, line) in utils::io::read_lines(input_file)?
        .map_while(Result::ok)
        .enumerate()
    {
        for caps in HEX.captures_iter(&line) {
            if let Some(m) = caps.get(0) {
                let hex_code = m.as_str().to_lowercase();
                insert_color_info(line_number, m, HexOrRgb::Hex(hex_code));
            }
        }

        for caps in RGB.captures_iter(&line) {
            if let Some(m) = caps.get(0) {
                let (Some(r), Some(g), Some(b)) =
                    (parse(&caps, 1), parse(&caps, 2), parse(&caps, 3))
                else {
                    continue;
                };

                insert_color_info(line_number, m, HexOrRgb::Rgb(Rgb::from(r, g, b)));
            }
        }

        for caps in RGB_ALPHA.captures_iter(&line) {
            if let Some(m) = caps.get(0) {
                let (Some(r), Some(g), Some(b), Some(a)) = (
                    parse(&caps, 1),
                    parse(&caps, 2),
                    parse(&caps, 3),
                    parse(&caps, 4),
                ) else {
                    continue;
                };

                let rgb = Rgb::from(r, g, b).set_alpha(a);
                insert_color_info(line_number, m, HexOrRgb::Rgb(rgb));
            }
        }

        for caps in HSL.captures_iter(&line) {
            if let Some(m) = caps.get(0) {
                let Some(h) = parse(&caps, 1) else {
                    continue;
                };

                if !(0.0..=360.0).contains(&h) {
                    continue;
                }

                let (Some(s), Some(l)) = (parse(&caps, 2), parse(&caps, 3)) else {
                    continue;
                };

                let rgb = Hsl::from(h, s, l).to_rgb();
                insert_color_info(line_number, m, HexOrRgb::Rgb(rgb));
            }
        }

        for caps in HSL_ALPHA.captures_iter(&line) {
            if let Some(m) = caps.get(0) {
                let Some(h) = parse(&caps, 1) else {
                    continue;
                };

                if !(0.0..=360.0).contains(&h) {
                    continue;
                }

                let (Some(s), Some(l), Some(a)) =
                    (parse(&caps, 2), parse(&caps, 3), parse(&caps, 4))
                else {
                    continue;
                };

                let rgb = Hsl::from(h, s, l).set_alpha(a).to_rgb();
                insert_color_info(line_number, m, HexOrRgb::Rgb(rgb));
            }
        }
    }

    Ok(p)
}

fn parse<T: std::str::FromStr>(caps: &regex::Captures, i: usize) -> Option<T> {
    caps.get(i).and_then(|m| m.as_str().parse::<T>().ok())
}

#[async_trait::async_trait]
impl ClapPlugin for ColorizerPlugin {
    async fn handle_action(&mut self, action: PluginAction) -> Result<(), PluginError> {
        match self.parse_action(&action.method)? {
            ColorizerAction::Toggle => {
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
            ColorizerAction::Off => {
                let bufnr = self.vim.bufnr("").await?;
                self.vim
                    .exec("clap#plugin#colorizer#clear_highlights", bufnr)?;
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_hex(text: &str) -> Vec<String> {
        HEX.captures_iter(text)
            .filter_map(|caps| caps.get(0).map(|m| m.as_str().to_lowercase()))
            .collect()
    }

    fn parse_rgb(text: &str) -> Vec<(usize, usize, usize)> {
        RGB.captures_iter(text)
            .filter_map(|caps| {
                if let (Some(r), Some(g), Some(b)) =
                    (parse(&caps, 1), parse(&caps, 2), parse(&caps, 3))
                {
                    Some((r, g, b))
                } else {
                    None
                }
            })
            .collect()
    }

    fn parse_rgb_alpha(text: &str) -> Vec<(usize, usize, usize, f64)> {
        RGB_ALPHA
            .captures_iter(text)
            .filter_map(|caps| {
                if let (Some(r), Some(g), Some(b), Some(a)) = (
                    parse(&caps, 1),
                    parse(&caps, 2),
                    parse(&caps, 3),
                    parse(&caps, 4),
                ) {
                    Some((r, g, b, a))
                } else {
                    None
                }
            })
            .collect()
    }

    fn parse_hsl(text: &str) -> Vec<(f32, f32, f32)> {
        HSL.captures_iter(text)
            .filter_map(|caps| {
                if let (Some(h), Some(s), Some(l)) =
                    (parse(&caps, 1), parse(&caps, 2), parse(&caps, 3))
                {
                    if (0.0..=360.0).contains(&h) {
                        Some((h, s, l))
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .collect()
    }

    fn parse_hsl_alpha(text: &str) -> Vec<(f32, f32, f32, f32)> {
        HSL_ALPHA
            .captures_iter(text)
            .filter_map(|caps| {
                if let (Some(h), Some(s), Some(l), Some(a)) = (
                    parse(&caps, 1),
                    parse(&caps, 2),
                    parse(&caps, 3),
                    parse(&caps, 4),
                ) {
                    if (0.0..=360.0).contains(&h) {
                        Some((h, s, l, a))
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .collect()
    }

    #[test]
    fn test_color_patterns() {
        let line = r#"#000 #00005f # 0000d7 0000ff #000#ae90d7 #FFF"#;

        assert_eq!(
            parse_hex(line),
            vec!["#000", "#00005f", "#000", "#ae90d7", "#fff"]
        );

        let line = r#"rgb(0, 12, 234) rgb(0,12,234) rgb(0,12,   234)"#;
        assert_eq!(
            parse_rgb(line),
            vec![(0, 12, 234), (0, 12, 234), (0, 12, 234)]
        );

        let line = r#"rgba(0, 12, 234, 0.5)"#;
        assert_eq!(parse_rgb_alpha(line), vec![(0, 12, 234, 0.5)]);

        let line = r#"hsl(0, 0%, 0%) hsl(195,75%,50%) hsl(195.5, 75.3%, 50.5%) hsl(360, 12%, 50%) hsl(500, 12%, 50%)"#;
        assert_eq!(
            parse_hsl(line),
            vec![
                (0.0, 0.0, 0.0),
                (195.0, 75.0, 50.0),
                (195.5, 75.3, 50.5),
                (360.0, 12.0, 50.0)
            ]
        );

        let line = r#"hsla(0, 0%, 0%, 0.3) hsla(360, 12%, 50%, 0.5)"#;
        assert_eq!(
            parse_hsl_alpha(line),
            vec![(0.0, 0.0, 0.0, 0.3), (360.0, 12.0, 50.0, 0.5)]
        );
    }
}
