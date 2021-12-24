use crate::Score;

fn find_start_at(slice: &str, start_at: usize, pat: &str) -> Option<usize> {
    slice[start_at..].find(pat).map(|i| start_at + i)
}

fn _substr_indices_impl(haystack: &str, niddle: &str) -> Option<(f64, Vec<usize>)> {
    let niddle = niddle.to_lowercase();

    if let Some(idx) = find_start_at(haystack, 0, &niddle) {
        let mut positions = Vec::new();

        // For build without overflow checks this could be written as
        // `let mut pos = idx - 1;` with `|| { pos += 1; pos }` closure.
        let mut pos = idx;
        positions.resize_with(
            niddle.len(),
            // Simple endless iterator for `idx..` range. Even though it's endless,
            // it will iterate only `sub_niddle.len()` times.
            || {
                pos += 1;
                pos - 1
            },
        );

        if let Some(last_pos) = positions.last() {
            let match_len = (last_pos + 1 - positions[0]) as f64;

            let score =
                (2f64 / (positions[0] + 1) as f64) + 1f64 / (last_pos + 1) as f64 - match_len;

            return Some((score, positions));
        }
    }

    None
}

fn unordered_substr_indices_impl(haystack: &str, niddle: &str) -> Option<(f64, Vec<usize>)> {
    // unreasonably large haystack
    if haystack.len() > 1024 {
        return None;
    }

    let haystack = haystack.to_lowercase();
    let haystack = haystack.as_str();

    let mut total_score = 0f64;
    let mut positions = Vec::new();
    for sub_niddle in niddle.split_whitespace() {
        if let Some((score, indices)) = _substr_indices_impl(haystack, sub_niddle) {
            total_score += score;
            positions.extend_from_slice(&indices);
        } else {
            return None;
        }
    }

    if positions.is_empty() {
        return Some((0f64, positions));
    }

    positions.sort_unstable();

    Some((total_score, positions))
}

pub fn substr_indices(haystack: &str, niddle: &str) -> Option<(Score, Vec<usize>)> {
    unordered_substr_indices_impl(haystack, niddle)
        .map(|(score, positions)| (score as Score, positions))
}

#[test]
fn test_substr() {
    assert_eq!(
        substr_indices("src/bun/blune", "sr bl"),
        Some((-1, vec![0, 1, 8, 9]))
    );

    assert_eq!(
        substr_indices("src/bun/blune", "bl sr"),
        Some((-1, vec![0, 1, 8, 9]))
    );
}
