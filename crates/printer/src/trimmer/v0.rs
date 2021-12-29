const DOTS: &str = "..";

// https://stackoverflow.com/questions/51982999/slice-a-string-containing-unicode-chars
#[inline]
pub fn utf8_str_slice(line: &str, start: usize, end: usize) -> String {
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
            if indices[0] - start >= DOTS.len() && line_len - start >= container_width {
                start += DOTS.len();
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
            format!("{}{}{}", icon, DOTS, utf8_str_slice(line, start, end))
        } else {
            format!("{}{}", DOTS, utf8_str_slice(line, start, end))
        };

        let offset = line_len.saturating_sub(left_truncated.len());

        let left_truncated_len = left_truncated.len();

        let (truncated, max_index) = if left_truncated_len > container_width {
            if left_truncated_len == container_width + 1 {
                (
                    format!("{}.", utf8_str_slice(&left_truncated, 0, container_width - 1)),
                    container_width - 1,
                )
            } else {
                (
                    format!(
                        "{}{}",
                        utf8_str_slice(&left_truncated, 0, container_width - 2),
                        DOTS
                    ),
                    container_width - 2,
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
