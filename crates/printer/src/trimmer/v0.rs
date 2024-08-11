//! This implementation has been deprecated, but still used in the Python
//! binding due to an unknown issue with the v1 implementation, to reproduce:
//!
//! 1. `let g:clap_force_python = 1`.
//! 2. open https://github.com/subspace/subspace/blob/c50bec907ab8ade923a2a0b4888f43bfc47e8a7f/polkadot/node/collation-generation/src/lib.rs
//! 3. Type `sr` and then you'll see Neovim hang forever, have no idea&time to fix it
//!    properly therefore the old implementation are just kept.

use super::AsciiDots;

// https://stackoverflow.com/questions/51982999/slice-a-string-containing-unicode-chars
#[inline]
fn utf8_str_slice(line: &str, start: usize, end: usize) -> String {
    line.chars().take(end).skip(start).collect()
}

pub fn trim_text(
    line: &str,
    indices: &[usize],
    container_width: usize,
    skipped: Option<usize>,
) -> Option<(String, Vec<usize>)> {
    let last_idx = indices.last()?;
    if *last_idx > container_width {
        let mut start = *last_idx - container_width;
        if start >= indices[0] || (indices.len() > 1 && *last_idx - start > container_width) {
            start = indices[0];
        }
        let line_len = line.len();
        // [--------------------------]
        // [-----------------------------------------------------------------xx--x--]
        for _ in 0..3 {
            if indices[0] - start >= AsciiDots::CHAR_LEN && line_len - start >= container_width {
                start += AsciiDots::CHAR_LEN;
            } else {
                break;
            }
        }
        let trailing_dist = line_len - last_idx;
        if trailing_dist < indices[0] - start {
            start += trailing_dist;
        }
        let end = line.len();
        let left_truncated = if let Some(n) = skipped {
            let icon: String = line.chars().take(n).collect();
            format!(
                "{}{}{}",
                icon,
                AsciiDots::DOTS,
                utf8_str_slice(line, start, end)
            )
        } else {
            format!("{}{}", AsciiDots::DOTS, utf8_str_slice(line, start, end))
        };

        let offset = line_len.saturating_sub(left_truncated.len());

        let left_truncated_len = left_truncated.len();

        let (truncated, max_index) = if left_truncated_len > container_width {
            if left_truncated_len == container_width + 1 {
                let left_truncated = utf8_str_slice(&left_truncated, 0, container_width - 1);
                (format!("{left_truncated}."), container_width - 1)
            } else {
                let left_truncated = utf8_str_slice(&left_truncated, 0, container_width - 2);
                (
                    format!("{left_truncated}{}", AsciiDots::DOTS),
                    container_width - AsciiDots::CHAR_LEN,
                )
            }
        } else {
            (left_truncated, container_width)
        };

        let truncated_indices = indices
            .iter()
            .map(|x| x - offset)
            .take_while(|x| *x < max_index)
            .collect::<Vec<_>>();

        Some((truncated, truncated_indices))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::utf8_str_slice;

    #[test]
    fn test_print_multibyte_string_slice() {
        let multibyte_str = "README.md:23:1:Gourinath Banda. “Scalable Real-Time Kernel for Small Embedded Systems”. En- glish. PhD thesis. Denmark: University of Southern Denmark, June 2003. URL: http://citeseerx.ist.psu.edu/viewdoc/download;jsessionid=84D11348847CDC13691DFAED09883FCB?doi=10.1.1.118.1909&rep=rep1&type=pdf.";
        let start = 33;
        let end = 300;
        let expected = "Scalable Real-Time Kernel for Small Embedded Systems”. En- glish. PhD thesis. Denmark: University of Southern Denmark, June 2003. URL: http://citeseerx.ist.psu.edu/viewdoc/download;jsessionid=84D11348847CDC13691DFAED09883FCB?doi=10.1.1.118.1909&rep=rep1&type=pdf.";
        assert_eq!(expected, utf8_str_slice(multibyte_str, start, end));
    }
}
