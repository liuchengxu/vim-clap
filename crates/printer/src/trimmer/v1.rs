use super::UnicodeDots;
use unicode_width::UnicodeWidthChar;

/// Returns the displayed width in columns of a `text`.
fn display_width(text: &str, tabstop: usize) -> usize {
    let mut w = 0;
    for ch in text.chars() {
        w += if ch == '\t' {
            tabstop - (w % tabstop)
        } else {
            ch.width().unwrap_or(2)
        };
    }
    w
}

/// Return an array in which arr[i] stores the display width till char[i] for `text`.
fn accumulate_text_width(text: &str, tabstop: usize) -> Vec<usize> {
    let mut ret = Vec::with_capacity(text.chars().count());
    let mut w = 0;
    for ch in text.chars() {
        w += if ch == '\t' {
            tabstop - (w % tabstop)
        } else {
            ch.width().unwrap_or(2)
        };
        ret.push(w);
    }
    ret
}

fn remove_first_char(value: &str) -> &str {
    let mut chars = value.chars();
    chars.next();
    chars.as_str()
}

/// `String` -> `..ring`.
///
/// Returns the original text with the left part trimmed and the length of trimmed text in chars.
fn trim_left(text: &str, width: usize, tabstop: usize) -> (&str, usize) {
    // Assume each char takes at least one column
    let chars_count = text.chars().count();
    let (mut text, mut trimmed_chars_len) = if chars_count > width + UnicodeDots::CHAR_LEN {
        let diff = chars_count - width - UnicodeDots::CHAR_LEN;
        // 292                             tracing::error!(error = ?e, "💔 Error at initializing GTAGS, attempting to recreate...");, 56, 4
        // thread 'main' panicked at 'byte index 62 is not a char boundary; it is inside '💔' (bytes 61..65) of `292                             tracing::error!(error = ?e, "💔 Error at initializing GTAGS, attempting to recreate...");`', library/core/src/str/mod.rs:127:5
        //
        // Can not use `(String::from(&text[diff..]), diff)` due to diff could not a char boundary.
        let mut chars = text.chars();
        (0..diff).for_each(|_| {
            chars.next();
        });
        (chars.as_str(), diff)
    } else {
        (text, 0)
    };

    let mut current_width = display_width(text, tabstop);

    while current_width > width && !text.is_empty() {
        text = remove_first_char(text);
        trimmed_chars_len += 1;
        current_width = display_width(text, tabstop);
    }

    (text, trimmed_chars_len)
}

/// `String` -> `Stri..`.
fn trim_right(text: &str, width: usize, tabstop: usize) -> &str {
    let current_width = display_width(text, tabstop);

    if current_width > width {
        if text.is_char_boundary(width) {
            &text[..width]
        } else {
            let mut width = width;
            while !text.is_char_boundary(width) {
                width -= 1;
            }
            &text[..width]
        }
    } else {
        text
    }
}

#[derive(Debug)]
pub enum TrimInfo {
    // ..ring
    Left { start: usize },
    // Stri..
    Right,
    // ..ri..
    Both { start: usize },
}

impl TrimInfo {
    pub fn left_trim_start(&self) -> Option<usize> {
        match self {
            Self::Left { start } | Self::Both { start } => Some(*start),
            _ => None,
        }
    }
}

#[derive(Debug)]
pub struct TrimmedText {
    // Trimmed text with dots.
    pub trimmed_text: String,
    pub indices: Vec<usize>,
    pub trim_info: TrimInfo,
}

/// Returns the potential trimmed text.
///
/// In order to make the highlights of matches visible in the container as much as possible,
/// both the left and right of the original text can be trimmed.
///
/// For example, if the matches appear in the end of a long string, we should trim the left and
/// only show the right part.
///
/// ```text
/// xxxxxxxxxxxxxxxxxxxxxxxxxxMMxxxxxMxxxxx
///               shift ->|               |
/// ```
///
/// container_width = winwidth - prefix_length
///
/// # Arguments
///
/// - `text`: original untruncated text.
/// - `indices`: highlights in char-positions.
/// - `container_width`: the width of window to display the text.
pub fn trim_text(
    text: &str,
    indices: &[usize],
    container_width: usize,
    tabstop: usize,
) -> Option<TrimmedText> {
    let match_start = indices[0];
    let match_end = *indices.last()?;

    let acc_width = accumulate_text_width(text, tabstop);

    // Width needed for diplaying the whole text.
    let full_width = *acc_width.last()?;

    if full_width <= container_width {
        return None;
    }

    //  xxxxxxxxxxxxxxxxxxxxMMxxxxxMxxxxxMMMxxxxxxxxxxxx
    // |<-      w1       ->|<-    w2     ->|<-  w3   ->|
    //
    // w1, w2, w3 = len_before_matched, len_matched, len_after_matched
    let w1 = if match_start == 0 {
        0
    } else {
        acc_width[match_start - 1]
    };

    let w2 = if match_end >= acc_width.len() {
        full_width - w1
    } else {
        acc_width[match_end] - w1
    };

    let w3 = full_width - w1 - w2;

    if (w1 > w3 && w2 + w3 <= container_width) || (w3 <= 2) {
        // right-fixed, ..ring
        let (trimmed_text, trimmed_len) =
            trim_left(text, container_width - UnicodeDots::CHAR_LEN, tabstop);

        let trimmed_text = format!("{}{trimmed_text}", UnicodeDots::DOTS);
        let indices = indices
            .iter()
            .filter_map(|x| (x + UnicodeDots::CHAR_LEN).checked_sub(trimmed_len))
            .filter(|x| *x > UnicodeDots::CHAR_LEN - 1) // Ignore the highlights in `..`
            .collect();

        Some(TrimmedText {
            trimmed_text,
            indices,
            trim_info: TrimInfo::Left { start: trimmed_len },
        })
    } else if w1 <= w3 && w1 + w2 <= container_width {
        // left-fixed, Stri..
        let trimmed_text = trim_right(text, container_width - UnicodeDots::CHAR_LEN, tabstop);

        let trimmed_text = format!("{trimmed_text}{}", UnicodeDots::DOTS);
        let indices = indices
            .iter()
            .filter(|x| *x + UnicodeDots::CHAR_LEN < container_width) // Ignore the highlights in `..`
            .copied()
            .collect::<Vec<_>>();

        Some(TrimmedText {
            trimmed_text,
            indices,
            trim_info: TrimInfo::Right,
        })
    } else {
        // Convert the char-position to byte-position.
        let match_start_byte_idx = text.char_indices().nth(match_start)?.0;

        // left-right, ..Stri..
        let left_truncated_text = &text[match_start_byte_idx..];
        let trimmed_text = trim_right(
            left_truncated_text,
            container_width - UnicodeDots::CHAR_LEN - UnicodeDots::CHAR_LEN,
            tabstop,
        );

        let trimmed_text = format!("{}{trimmed_text}{}", UnicodeDots::DOTS, UnicodeDots::DOTS);
        let indices = indices
            .iter()
            .map(|x| x - match_start + UnicodeDots::CHAR_LEN)
            .filter(|x| *x + UnicodeDots::CHAR_LEN < container_width) // Ignore the highlights in `..`
            .collect::<Vec<_>>();

        Some(TrimmedText {
            trimmed_text,
            indices,
            trim_info: TrimInfo::Both {
                start: match_start_byte_idx,
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::filter_single_line;
    use types::MatchedItem;

    const DOTS: char = UnicodeDots::DOTS;

    #[test]
    fn test_trim_left() {
        let text = "0123456789abcdef";
        let width = 5;
        let (trimmed, offset) = trim_left(text, width, 4);
        assert_eq!(trimmed, "bcdef");
        assert_eq!(offset, 11);
    }

    #[test]
    fn test_trim_right() {
        let text = "0123456789abcdef";
        let width = 5;
        let trimmed = trim_right(text, width, 4);
        assert_eq!(trimmed, "01234");
    }

    #[test]
    fn test_trim_text() {
        // raw_line, query, highlighted, container_width, display_line
        let test_cases = vec![
            (
                "directories/are/nested/a/lot/then/the/matched/items/will/be/invisible/file.scss",
                "files",
                "files",
                50usize,
                format!("{DOTS}hen/the/matched/items/will/be/invisible/file.scss"),
            ),
            (
                "directories/are/nested/a/lot/then/the/matched/items/will/be/invisible/another-file.scss",
                "files",
                "files",
                50usize,
                format!("{DOTS}matched/items/will/be/invisible/another-file.scss"),
            ),
            (
                "directories/are/nested/a/lot/then/the/matched/items/will/be/invisible/file.js",
                "files",
                "files",
                50usize,
                format!("{DOTS}/then/the/matched/items/will/be/invisible/file.js"),
            ),
            (
                "directories/are/nested/a/lot/then/the/matched/items/will/be/invisible/another-file.js",
                "files",
                "files",
                50usize,
                format!("{DOTS}e/matched/items/will/be/invisible/another-file.js"),
            ),
            (
                "/Users/xuliucheng/Library/Caches/Homebrew/universal-ctags--git/Units/afl-fuzz.r/github-issue-625-r.d/input.r",
                "srcggithub",
                "srcg",
                50usize,
                format!("{DOTS}s/Homebrew/universal-ctags--git/Units/afl-fuzz.r{DOTS}"),
            ),
            (
                "        // Wait until propagation delay period after block we plan to mine on",
                "bmine",
                "bmine",
                58usize,
                format!("{DOTS}l propagation delay period after block we plan to mine on"),
            ),
            (
                "fuzzy-filter/target/debug/deps/librustversion-b273394e6c9c64f6.dylib.dSYM/Contents/Resources/DWARF/librustversion-b273394e6c9c64f6.dylib",
                "srlisresource",
                "srlisR",
                50usize,
                format!("{DOTS}stversion-b273394e6c9c64f6.dylib.dSYM/Contents/R{DOTS}"),
            ),
            (
                "target/debug/deps/libstructopt_derive-3921fbf02d8d2ffe.dylib.dSYM/Contents/Resources/DWARF/libstructopt_derive-3921fbf02d8d2ffe.dylib",
                "srlisresource",
                "srli",
                50usize,
                format!("{DOTS}structopt_derive-3921fbf02d8d2ffe.dylib.dSYM/Con{DOTS}")
            ),
            (
                "fuzzy-filter/target/debug/deps/librustversion-15764ff2535f190d.dylib.dSYM/Contents/Resources/DWARF/librustversion-15764ff2535f190d.dylib",
                "srlisresource",
                "srlisR",
                50usize,
                format!("{DOTS}stversion-15764ff2535f190d.dylib.dSYM/Contents/R{DOTS}")
            ),
            (
                "crates/readtags/sys/libreadtags/autom4te.cache/requests",
                "srlisrs",
                "lisrs",
                42usize,
                format!("{DOTS}s/sys/libreadtags/autom4te.cache/requests")
            ),
            (
                "crates/maple_cli/src/dumb_analyzer/find_usages/default_types.rs",
                "srlisrs",
                "lisrs",
                42usize,
                format!("{DOTS}umb_analyzer/find_usages/default_types.rs")
            ),
            (
                r#"crates/printer/src/lib.rs:312:4:" crates/fuzzy_filter/target/debug/deps/librustversion-15764ff2535f190d.dylib.dSYM/Contents/Resources/DWARF/librustversion-15764ff2535f190d.dylib"#,
                "ctagslisr",
                "ctagsli",
                80usize,
                format!("{DOTS}crates/fuzzy_filter/target/debug/deps/librustversion-15764ff2535f190d.dylib.dS{DOTS}")
            ),
        ];

        for (text, query, highlighted, container_width, display_line) in test_cases {
            let ranked = filter_single_line(text.to_string(), query);

            let MatchedItem { indices, .. } = ranked[0].clone();

            let (display_line_got, indices_post) = trim_text(text, &indices, container_width, 4)
                .map(|trimmed| (trimmed.trimmed_text, trimmed.indices))
                .unwrap_or_else(|| (text.into(), indices.clone()));

            let truncated_text_got = display_line_got.clone();

            let highlighted_got = indices_post
                .iter()
                .filter_map(|i| truncated_text_got.chars().nth(*i))
                .collect::<String>();

            assert_eq!(display_line, display_line_got);
            assert_eq!(highlighted, highlighted_got);
        }
    }
}
