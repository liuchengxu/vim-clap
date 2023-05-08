use colorsys::Rgb;
use highlighter::SyntaxHighlighter;
use rgb2ansi256::rgb_to_ansi256;
use std::ops::Range;
use syntect::easy::HighlightLines;
use syntect::highlighting::{Style, ThemeSet};
use syntect::parsing::SyntaxSet;
use syntect::util::{as_24_bit_terminal_escaped, as_latex_escaped, LinesWithEndings};

fn main() {
    let highlighter = SyntaxHighlighter::new();

    let lines = vec![
        "pub fn highlight_line(&self, extension: &str, line: &str) -> Vec<HighlightToken> {",
        "    let syntax = self.syntax_set.find_syntax_by_extension(extension).unwrap();",
        r#"    // let mut h = HighlightLines::new(syntax, &self.theme_set.themes["Solarized (light)"]);"#,
        r#"    let mut h = HighlightLines::new(syntax, &self.theme_set.themes["Solarized (dark)"]);"#,
        "}",
    ];

    for line in lines {
        highlighter.highlight_line("rs", line);
    }

    /*
    println!("themes: {:?}", ts.themes.keys());

    let s = "pub struct Wow { hi: u64 }\nfn blah() -> u64 {}\n";

    // let mut h = HighlightLines::new(syntax, &ts.themes["InspiredGitHub"]);
    let mut h = HighlightLines::new(syntax, &ts.themes["Solarized (dark)"]);
    for line in LinesWithEndings::from(s) {
        // LinesWithEndings enables use of newlines mode
        let ranges: Vec<(Style, &str)> = h.highlight_line(line, &ps).unwrap();

        let mut offset = 0;
        let highlight_tokens = ranges
            .iter()
            .filter_map(|(style, text)| {
                let chars_count = text.chars().count();
                offset += chars_count;
                if text.trim().is_empty() {
                    None
                } else {
                    Some(HighlightToken {
                        highlight_args: HighlightArgs::from_style(style),
                        range: (offset - chars_count..offset),
                    })
                }
            })
            .collect::<Vec<_>>();

        for highlight_token in &highlight_tokens {
            println!(
                "range: {:?}, text: {:?}, color: fg{}, bg{}",
                highlight_token.range,
                &line[highlight_token.range.start..highlight_token.range.end],
                highlight_token.highlight_args.guifg.to_hex_string(),
                highlight_token.highlight_args.guibg.to_hex_string(),
            );
        }

        println!("ranges: {ranges:?}");
        // println!("highlight_ranges: {highlight_ranges:?}");
        // let escaped = as_latex_escaped(&ranges[..]);
        let escaped = as_24_bit_terminal_escaped(&ranges[..], false);
        println!("\n{:?}", line);
        println!("\n{}", escaped);
    }
    */
}
