//! Working with utf8-encoded strings.

use super::{
    score_with_positions,
    scoring_utils::{MatchWithPositions, Score},
    FzyItem,
};

#[inline]
pub fn match_and_score_with_positions(
    needle: &str,
    haystack: &str,
    prealloced_matricies: &mut (Vec<Score>, Vec<Score>),
) -> Option<MatchWithPositions> {
    match matches(needle, haystack) {
        Some(needle_length) => {
            let (score, positions) =
                score_with_positions(needle, needle_length, haystack, prealloced_matricies);
            Some((score, positions))
        }
        None => None,
    }
}

/// Searches for needle's chars in the haystack.
/// Returns `None` if haystack doesn't hold all needle's chars.
/// Returns `Some(len)` with needle's length otherwise.
///
/// # Examples
///
// This is a proper code, that should compile, but `matches()` function is private.
/// ```ignore
/// assert_eq!(Some(5), extracted_fzy::matches("amo汉漢", "app/models/order/汉语/漢語"));
/// assert_eq!(6, "汉漢".len()); // Length of this two chars in bytes.
/// ```
#[inline]
fn matches(needle: &str, haystack: &str) -> Option<usize> {
    if needle.is_empty() || needle == haystack {
        return Some(0);
    }

    let mut hchars = haystack.chars();

    // Use loop instead of `needle.all()`, to count needle's length.
    let mut needle_length = 0;
    for n in needle.chars() {
        if !hchars.any(|h| FzyItem::eq(n, h)) {
            return None;
        }
        needle_length += 1;
    }
    Some(needle_length)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn abc() {
        let abc = "abc";
        let cba = "cba";
        let res = match_and_score_with_positions(abc, cba, &mut Default::default());
        // assert!(res.is_some());
        assert!(res.is_none());
    }
}
