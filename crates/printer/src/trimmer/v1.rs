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

/// `String` -> `..ring`.
fn trim_left(text: &str, width: usize, tabstop: usize) -> (String, usize) {
    // Assume each char takes at least one column
    let chars_count = text.chars().count();
    let (mut text, mut trimmed_len) = if chars_count > width + 2 {
        let diff = chars_count - width - 2;
        (String::from(&text[diff..]), diff)
    } else {
        (text.into(), 0)
    };

    let mut current_width = display_width(&text, tabstop);

    while current_width > width && !text.is_empty() {
        text = text.chars().skip(1).collect();
        trimmed_len += 1;
        current_width = display_width(&text, tabstop);
    }

    (text, trimmed_len)
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

/// Trim the left and right of origin text accordingly to make it fit into the container.
///
/// For example, if the match appear in the end of a long string, we need to show the right part.
///
/// ```text
/// xxxxxxxxxxxxxxxxxxxxxxxxxxMMxxxxxMxxxxx
///               shift ->|               |
/// ```
///
/// container_width = winwidth - prefix_length
pub fn trim_text(
    text: &str,
    indices: &[usize],
    container_width: usize,
    tabstop: usize,
) -> Option<(String, Vec<usize>)> {
    let match_start = indices[0];
    let match_end = *indices.last().expect("indices are non empty; qed");

    let acc_width = accumulate_text_width(text, tabstop);

    // Width for diplaying the whole text.
    let full_width = *acc_width.last().expect("`acc_width` is not empty; qed");

    if full_width <= container_width {
        return None;
    }

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
        let (trimmed_text, trimmed_len) = trim_left(text, container_width - 2, tabstop);

        let text = format!("..{trimmed_text}");
        let indices = indices
            .iter()
            .filter_map(|x| (x + 2).checked_sub(trimmed_len))
            .filter(|x| *x > 1) // Ignore the highlights in `..`
            .collect();

        Some((text, indices))
    } else if w1 <= w3 && w1 + w2 <= container_width {
        // left-fixed, Stri..
        let trimmed_text = trim_right(text, container_width - 2, tabstop);

        let text = format!("{trimmed_text}..");
        let indices = indices
            .iter()
            .filter(|x| *x + 2 < container_width) // Ignore the highlights in `..`
            .copied()
            .collect::<Vec<_>>();

        Some((text, indices))
    } else {
        // left-right, ..Stri..
        let left_truncated_text = &text[match_start..];
        let trimmed_text = trim_right(left_truncated_text, container_width - 2 - 2, tabstop);

        let text = format!("..{trimmed_text}..");
        let indices = indices
            .iter()
            .map(|x| x - match_start + 2)
            .filter(|x| *x + 2 < container_width) // Ignore the highlights in `..`
            .collect::<Vec<_>>();

        Some((text, indices))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::filter_single_line;
    use types::MatchedItem;

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
        let test_cases = vec![(
            // raw_line, query, highlighted, container_width, display_line
            "directories/are/nested/a/lot/then/the/matched/items/will/be/invisible/file.scss",
            "files", "files", 50usize,
            "..en/the/matched/items/will/be/invisible/file.scss",
          ),
          (
            "directories/are/nested/a/lot/then/the/matched/items/will/be/invisible/another-file.scss",
            "files", "files", 50usize,
            "..atched/items/will/be/invisible/another-file.scss",
          ),
          (
          "directories/are/nested/a/lot/then/the/matched/items/will/be/invisible/file.js",
          "files", "files", 50usize,
          "..then/the/matched/items/will/be/invisible/file.js",
          ),
          (
          "directories/are/nested/a/lot/then/the/matched/items/will/be/invisible/another-file.js",
          "files", "files", 50usize,
          "../matched/items/will/be/invisible/another-file.js",
          ),
          (
            "/Users/xuliucheng/Library/Caches/Homebrew/universal-ctags--git/Units/afl-fuzz.r/github-issue-625-r.d/input.r",
            "srcggithub", "srcg", 50usize,
            "..s/Homebrew/universal-ctags--git/Units/afl-fuzz..",
          ),
          (
            "        // Wait until propagation delay period after block we plan to mine on",
            "bmine", "bmine", 58usize,
            ".. propagation delay period after block we plan to mine on"
          ),
          (
            "fuzzy-filter/target/debug/deps/librustversion-b273394e6c9c64f6.dylib.dSYM/Contents/Resources/DWARF/librustversion-b273394e6c9c64f6.dylib",
            "srlisresource", "srlis", 50usize,
            "..stversion-b273394e6c9c64f6.dylib.dSYM/Contents.."
          ),
          (
            "target/debug/deps/libstructopt_derive-3921fbf02d8d2ffe.dylib.dSYM/Contents/Resources/DWARF/libstructopt_derive-3921fbf02d8d2ffe.dylib",
            "srlisresource", "srli", 50usize,
            "..structopt_derive-3921fbf02d8d2ffe.dylib.dSYM/C..",
          ),
          (
            "fuzzy-filter/target/debug/deps/librustversion-15764ff2535f190d.dylib.dSYM/Contents/Resources/DWARF/librustversion-15764ff2535f190d.dylib",
            "srlisresource", "srlis", 50usize,
            "..stversion-15764ff2535f190d.dylib.dSYM/Contents..",
          ),
            (
              "crates/readtags/sys/libreadtags/autom4te.cache/requests",
              "srlisrs", "lisrs", 42usize,
              "../sys/libreadtags/autom4te.cache/requests"
            ),
              (
              "crates/maple_cli/src/dumb_analyzer/find_usages/default_types.rs",
              "srlisrs", "lisrs", 42usize,
              "..mb_analyzer/find_usages/default_types.rs"
              )
        ];

        for (text, query, highlighted, container_width, display_line) in test_cases {
            let ranked = filter_single_line(text.to_string(), query);

            let MatchedItem { match_indices, .. } = ranked[0].clone();

            let (display_line_got, indices_post) =
                trim_text(text, &match_indices, container_width, 4)
                    .unwrap_or((text.into(), match_indices.clone()));

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
