use std::path::Path;

use anyhow::Result;
use fuzzy_filter::{fuzzy_filter_and_rank, truncate_long_matched_lines, Algo, Source};

use icon::prepend_icon;

pub fn run<I: Iterator<Item = String>>(
    query: &str,
    source: Source<I>,
    algo: Option<Algo>,
    number: Option<usize>,
    enable_icon: bool,
    winwidth: Option<usize>,
) -> Result<()> {
    let ranked = fuzzy_filter_and_rank(query, source, algo.unwrap_or(Algo::Fzy))?;

    if let Some(number) = number {
        let total = ranked.len();
        let payload = ranked.into_iter().take(number);
        let winwidth = winwidth.unwrap_or(62);
        let (truncated_payload, truncated_map) =
            truncate_long_matched_lines(payload, winwidth, None);
        let mut lines = Vec::with_capacity(number);
        let mut indices = Vec::with_capacity(number);
        if enable_icon {
            for (text, _, idxs) in truncated_payload {
                let iconized = prepend_icon(&text);
                lines.push(iconized);
                indices.push(idxs);
            }
        } else {
            for (text, _, idxs) in truncated_payload {
                lines.push(text);
                indices.push(idxs);
            }
        }
        if truncated_map.is_empty() {
            println_json!(total, lines, indices);
        } else {
            println_json!(total, lines, indices, truncated_map);
        }
    } else {
        for (text, _, indices) in ranked.iter() {
            println_json!(text, indices);
        }
    }

    Ok(())
}

/// Looks for matches of `query` in lines of the current vim buffer.
pub fn blines(
    query: &str,
    input: &Path,
    number: Option<usize>,
    winwidth: Option<usize>,
) -> Result<()> {
    run(
        query,
        Source::List(
            std::fs::read_to_string(&input)?
                .lines()
                .enumerate()
                .map(|(idx, item)| format!("{} {}", idx + 1, item)),
        ),
        None,
        number,
        false,
        winwidth,
    )
}

/// Return the ranked results after applying fuzzy filter given the query string and a list of candidates.
pub fn dyn_fuzzy_filter_and_rank<I: Iterator<Item = String>>(
    query: &str,
    source: Source<I>,
    algo: Algo,
    number: Option<usize>,
    enable_icon: bool,
    winwidth: Option<usize>,
) -> Result<()> {
    use {
        fulf::utf8::match_and_score_with_positions,
        fuzzy_filter::FuzzyMatchedLineInfo,
        fuzzy_matcher::skim::fuzzy_indices,
        rayon::slice::ParallelSliceMut,
        std::io::{self, BufRead},
    };

    let scorer = |line: &str| match algo {
        Algo::Skim => fuzzy_indices(line, query),
        Algo::Fzy => match_and_score_with_positions(query, line)
            .map(|(score, indices)| (score as i64, indices)),
    };

    /// The constant to define the length of `top_` queues.
    const ITEMS_TO_SHOW: usize = 8;

    const MAX_IDX: usize = ITEMS_TO_SHOW - 1;

    trait Insert<T> {
        fn pop_and_insert(&mut self, idx: usize, value: T);
    }
    impl<T: Copy> Insert<T> for [T; ITEMS_TO_SHOW] {
        fn pop_and_insert(&mut self, idx: usize, value: T) {
            if idx < MAX_IDX {
                self.copy_within(idx..MAX_IDX, idx + 1);
                self[idx] = value;
            } else {
                self[MAX_IDX] = value;
            }
        }
    }

    /// This macro is a special thing for [`dyn_collect_all`] and [`dyn_collect_number`].
    macro_rules! insert_both {
            // This macro pushes all things into buffer, pops one worst item from each top queue
            // and then inserts all things into `top_` queues.
            (pop; $index:expr, $score:expr, $text:expr, $indices:expr => $buffer:expr, $top_results:expr, $top_scores:expr) => {{
                match $index {
                    // If index is last possible, then the worst item is better than this we want to push in,
                    // and we do nothing.
                    Some(MAX_IDX) => $buffer.push(($text, $score, $indices)),
                    // Else, one item gets popped from the queue
                    // and other is inserted.
                    Some(idx) => {
                        insert_both!(idx + 1, $score, $text, $indices => $buffer, $top_results, $top_scores);
                    }
                    None => {
                        insert_both!(0, $score, $text, $indices => $buffer, $top_results, $top_scores);
                    }
                }
            }};

            // This macro pushes all things into buffer and inserts all things into
            // `top_` queues.
            ($index:expr, $score:expr, $text:expr, $indices:expr => $buffer:expr, $top_results:expr, $top_scores:expr) => {{
                $buffer.push(($text, $score, $indices));
                $top_results.pop_and_insert($index, $buffer.len() - 1);
                $top_scores.pop_and_insert($index, $score);
            }};
        }

    /// To get dynamic updates, not so much should be changed, actually.
    /// First: instead of collecting iterator into vector, this iterator
    /// should be `for_each`ed or something like this.
    /// Second: while iterator is `for_each`ed, its results are collected
    /// into some collection, and `total` is increased by one for each iteration.
    /// At some points of iteration that collection gets printed and voila!
    ///
    /// Though it sounds easy, there's one pitfalls:
    /// `par_iter` iteration should use atomic `total`, because, well, it's parallel.
    /// And some rough edges: if there's too much results, sorting and json+print
    /// could take too much time. Same problem for too big `number`.
    ///
    /// So, to get dynamic results, I'm gonna use VecDeque with little constant space.
    /// But there's a problem with `par_iter` again, as there should be mutexed access to the
    /// VecDeque for this iterator.
    ///
    /// So, this particular function won't work in parallel context at all.
    fn dyn_collect_all(
        mut iter: impl Iterator<Item = FuzzyMatchedLineInfo>,
    ) -> Vec<FuzzyMatchedLineInfo> {
        let mut buffer = Vec::with_capacity({
            let (low, high) = iter.size_hint();
            high.unwrap_or(low)
        });

        let mut top_scores: [i64; ITEMS_TO_SHOW] = [i64::min_value(); ITEMS_TO_SHOW];
        let mut top_results: [usize; ITEMS_TO_SHOW] = [usize::min_value(); ITEMS_TO_SHOW];

        // First, let's try to produce `ITEMS_TO_SHOW` items to fill the topscores.
        let mut total = 0;
        let if_ok_return = iter.try_for_each(|(text, score, indices)| {
            // Best results are stored in front.
            //XXX I can't say, if bigger score is better or not. Let's assume the bigger the better.
            let idx = match top_scores
                .iter()
                .rev() // .rev(), because worse items are at the end.
                .enumerate()
                .find(|&(_, &other_score)| other_score > score)
            {
                Some((idx, _)) => idx + 1,
                None => 0,
            };
            insert_both!(idx, score, text, indices => buffer, top_results, top_scores);

            // Stop iterating after `ITEMS_TO_SHOW` iterations.
            total += 1;
            if total == ITEMS_TO_SHOW {
                Err(())
            } else {
                Ok(())
            }
        });

        if let Ok(()) = if_ok_return {
            return buffer;
        };

        // Now we have the full queue and can just pair `.pop_back()` with `.insert()` to keep
        // the queue with best results the same size.
        let mut past = std::time::Instant::now();
        iter.for_each(|(text, score, indices)| {
                // Best results are stored in front.
                //XXX I can't say, if bigger score is better or not. Let's assume the bigger the better.
                let idx = top_scores
                    .iter()
                    .rev() // .rev(), because worse items are at the end.
                    .enumerate()
                    .find(|&(_, &other_score)| other_score > score);
                insert_both!(pop; idx.map(|(i, _)| i), score, text, indices => buffer, top_results, top_scores);

                total = total.wrapping_add(1);
                if total % 16 == 0 {
                    use std::time::{Duration, Instant};

                    const UPDATE_INTERVAL: Duration = Duration::from_secs(2);

                    let now = Instant::now();
                    if now > past + UPDATE_INTERVAL {
                        past = now;
                        println_json!(total);
                        for &idx in top_results.iter() {
                            let (text, _, indices) = std::ops::Index::index(buffer.as_slice(), idx);
                            println_json!(text, indices);
                        }
                    }
                }
            });

        buffer
    }

    /// If you only need a `number` of elements, then you don't need to collect all
    /// items produced by the iterator.
    ///
    /// # Returns
    ///
    /// Tuple of `(total_number_of_iterations: usize, Vec<_>)`.
    /// The vector is not sorted nor truncated.
    //
    // Even though the current implementation isn't the most effective thing to do it,
    // I think, it's just good enough. And should be more effective than full
    // `collect()` into Vec on big numbers of iterations.
    fn dyn_collect_number(
        mut iter: impl Iterator<Item = FuzzyMatchedLineInfo>,
        number: usize,
    ) -> (usize, Vec<FuzzyMatchedLineInfo>) {
        // To not have problems with queues after sorting and truncating the buffer,
        // buffer has the lowest bound of `ITEMS_TO_SHOW * 2`, not `number * 2`.
        let mut buffer = Vec::with_capacity(2 * std::cmp::max(ITEMS_TO_SHOW, number));

        let mut top_scores: [i64; ITEMS_TO_SHOW] = [i64::min_value(); ITEMS_TO_SHOW];
        let mut top_results: [usize; ITEMS_TO_SHOW] = [usize::min_value(); ITEMS_TO_SHOW];

        // First, let's try to produce `ITEMS_TO_SHOW` items to fill the topscores.
        let mut total = 0;
        let if_ok_return = iter.try_for_each(|(text, score, indices)| {
            // Best results are stored in front.
            //XXX I can't say, if bigger score is better or not. Let's assume the bigger the better.
            let idx = match top_scores
                .iter()
                .rev() // .rev(), because worse items are at the end.
                .enumerate()
                .find(|&(_, &other_score)| other_score > score)
            {
                Some((idx, _)) => idx + 1,
                None => 0,
            };
            insert_both!(idx, score, text, indices => buffer, top_results, top_scores);

            // Stop iterating after `ITEMS_TO_SHOW` iterations.
            total += 1;
            if total == ITEMS_TO_SHOW {
                Err(())
            } else {
                Ok(())
            }
        });

        if let Ok(()) = if_ok_return {
            return (total, buffer);
        };

        // Now we have the full queue and can just pair `.pop_back()` with `.insert()` to keep
        // the queue with best results the same size.
        let mut past = std::time::Instant::now();
        iter.for_each(|(text, score, indices)| {
                // Best results are stored in front.
                //XXX I can't say, if bigger score is better or not. Let's assume the bigger the better.
                let idx = top_scores
                    .iter()
                    .rev() // .rev(), because worse items are at the end.
                    .enumerate()
                    .find(|&(_, &other_score)| other_score > score);
                insert_both!(pop; idx.map(|(i, _)| i), score, text, indices => buffer, top_results, top_scores);

                total += 1;
                if total % 16 == 0 {
                    use std::time::{Duration, Instant};

                    const UPDATE_INTERVAL: Duration = Duration::from_secs(2);

                    let now = Instant::now();
                    if now > past + UPDATE_INTERVAL {
                        past = now;
                        println_json!(total);
                        for &idx in top_results.iter() {
                            let (text, _, indices) = std::ops::Index::index( buffer.as_slice(), idx);
                            println_json!(text, indices);
                        }
                    }
                }

                if buffer.len() == buffer.capacity() {
                    buffer.par_sort_unstable_by(|(_, v1, _), (_, v2, _)| v2.partial_cmp(&v1).unwrap());

                    for (idx, (_, score, _)) in buffer[..ITEMS_TO_SHOW].iter().enumerate() {
                        top_scores[idx] = *score;
                        top_results[idx] = idx;
                    }

                    let half = buffer.len() / 2;
                    buffer.truncate(half);
                }
            });

        (total, buffer)
    }

    if let Some(number) = number {
        let (total, filtered) = match source {
            Source::Stdin => dyn_collect_number(
                io::stdin().lock().lines().filter_map(|lines_iter| {
                    lines_iter.ok().and_then(|line| {
                        scorer(&line).map(|(score, indices)| (line, score, indices))
                    })
                }),
                number,
            ),
            Source::File(fpath) => dyn_collect_number(
                std::fs::read_to_string(fpath)?.lines().filter_map(|line| {
                    scorer(&line).map(|(score, indices)| (line.into(), score, indices))
                }),
                number,
            ),
            Source::List(list) => dyn_collect_number(
                list.filter_map(|line| {
                    scorer(&line).map(|(score, indices)| (line, score, indices))
                }),
                number,
            ),
        };

        let ranked = filtered;

        let payload = ranked.into_iter().take(number);
        let winwidth = winwidth.unwrap_or(62);
        let (truncated_payload, truncated_map) =
            truncate_long_matched_lines(payload, winwidth, None);
        let mut lines = Vec::with_capacity(number);
        let mut indices = Vec::with_capacity(number);
        if enable_icon {
            for (text, _, idxs) in truncated_payload {
                let iconized = prepend_icon(&text);
                lines.push(iconized);
                indices.push(idxs);
            }
        } else {
            for (text, _, idxs) in truncated_payload {
                lines.push(text);
                indices.push(idxs);
            }
        }
        if truncated_map.is_empty() {
            println_json!(total, lines, indices);
        } else {
            println_json!(total, lines, indices, truncated_map);
        }
    } else {
        let mut filtered = match source {
            Source::Stdin => dyn_collect_all(io::stdin().lock().lines().filter_map(|lines_iter| {
                lines_iter
                    .ok()
                    .and_then(|line| scorer(&line).map(|(score, indices)| (line, score, indices)))
            })),
            Source::File(fpath) => {
                dyn_collect_all(std::fs::read_to_string(fpath)?.lines().filter_map(|line| {
                    scorer(line).map(|(score, indices)| (line.into(), score, indices))
                }))
            }
            Source::List(list) => {
                dyn_collect_all(list.filter_map(|line| {
                    scorer(&line).map(|(score, indices)| (line, score, indices))
                }))
            }
        };

        filtered.par_sort_unstable_by(|(_, v1, _), (_, v2, _)| v2.partial_cmp(&v1).unwrap());

        let ranked = filtered;

        for (text, _, indices) in ranked.iter() {
            println_json!(text, indices);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    // This is a very time-consuming test,
    // results of which could be proved only be inspecting stdout.
    // Definetly not something you want to run with `cargo test`.
    #[ignore]
    fn dynamic_results() {
        use std::time::{SystemTime, UNIX_EPOCH};

        const ALPHABET: [u8; 32] = [
            b'q', b'w', b'e', b'r', b't', b'y', b'u', b'i', b'o', b'p', b'[', b']', b'a', b's',
            b'd', b'f', b'g', b'h', b'j', b'k', b'l', b';', b'z', b'x', b'c', b'v', b'b', b'n',
            b'm', b',', b'.', b' ',
        ];

        // To mock the endless randomized text, we need three numbers:
        // 1. A number of letters to change.
        // then for each such number
        // 2. Alphabet index to get a new letter,
        // 3. Changing text index to write a new letter.
        let now = SystemTime::now();
        let mut bytes: usize = now
            .duration_since(UNIX_EPOCH)
            .unwrap_or_else(|_| UNIX_EPOCH.duration_since(now).unwrap())
            .as_secs() as usize;

        let mut changing_text: [u8; 16] = [ALPHABET[31]; 16];
        let mut total_lines_created: usize = 0;
        dyn_fuzzy_filter_and_rank(
            "abc",
            Source::List(
                std::iter::repeat_with(|| {
                    bytes = bytes.reverse_bits().rotate_right(3).wrapping_add(1);

                    let mut n = bytes;
                    // Number of letter to change.
                    let i = (n % 4) + 1;
                    n /= 4;
                    for _ in 0..i {
                        let text_idx = n % 16;
                        n /= 16;
                        let ab_idx = n % 32;
                        n /= 32;

                        changing_text[text_idx] = ALPHABET[ab_idx];
                    }

                    total_lines_created += 1;
                    if total_lines_created % 99999_usize.next_power_of_two() == 0 {
                        println!("Total lines created: {}", total_lines_created)
                    }

                    String::from_utf8(changing_text.as_ref().to_owned()).unwrap()
                })
                .take(usize::max_value() >> 8),
            ),
            Algo::Fzy,
            Some(100),
            false,
            None,
        )
        .unwrap()
    }
}
