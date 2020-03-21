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
        extracted_fzy::match_and_score_with_positions,
        fuzzy_filter::FuzzyMatchedLineInfo,
        fuzzy_matcher::skim::fuzzy_indices,
        rayon::slice::ParallelSliceMut,
        std::io::{self, BufRead},
    };

    let scorer = |line: &str| match algo {
        Algo::Skim => fuzzy_indices(line, query).map(|(score, indices)| (score as f64, indices)),
        Algo::Fzy => match_and_score_with_positions(query, line),
    };

    /// This macro is a special thing for [`dyn_collect_all`] and [`dyn_collect_number`].
    macro_rules! insert_both {
            // This macro pushes all things into buffer, pops one worst item from each top queue
            // and then inserts all things into `top_` queues.
            (pop; $index:expr, $score:expr, $text:expr, $indices:expr => $buffer:expr, $top_results:expr, $top_scores:expr) => {{
                match $index {
                    // If index is 0, then the worst item is better than this we want to push in,
                    // and we do nothing.
                    Some(0) => $buffer.push(($text, $score, $indices)),
                    // If index is not zero, then one item gets popped from the queue
                    // and other is inserted.
                    Some(idx) => {
                        $top_results.pop_back();
                        $top_scores.pop_back();
                        // `index - 1` because one item was popped
                        insert_both!(idx - 1, $score, $text, $indices => $buffer, $top_results, $top_scores);
                    }
                    None => {
                        $top_results.pop_back();
                        $top_scores.pop_back();
                        // `index - 1` because one item was popped
                        insert_both!(ITEMS_TO_SHOW - 1, $score, $text, $indices => $buffer, $top_results, $top_scores);
                    }
                }
            }};

            // This macro pushes all things into buffer and inserts all things into
            // `top_` queues.
            ($index:expr, $score:expr, $text:expr, $indices:expr => $buffer:expr, $top_results:expr, $top_scores:expr) => {{
                $buffer.push(($text, $score, $indices));
                // I just pushed the item into this buffer, lol, `unwrap()` can't fail.
                // let (text, _, indices) = buffer.last().unwrap();
                let last = $buffer.last().unwrap();
                //XXX SAFETY: as long as `top_results` isn't used after `buffer`
                //XXX starts to change it's elements' internals or is dropped,
                //XXX that's safe. When `buffer` is reallocated it can change
                //XXX the adress of data it holds but not internals of that data,
                //XXX so `text.as_str()` and `indxs.as_slice()`
                //XXX will never become invalidated on `buffer` reallocation,
                //XXX only on explicit mutability or drop of the item,
                //XXX or when buffer itself will get dropped.
                let (text_ref, indices_ref) = unsafe {
                    (
                        std::mem::transmute::<&str, &'static str>(last.0.as_str()),
                        std::mem::transmute::<&[_], &'static [_]>(last.2.as_slice()),
                    )
                };
                $top_results.insert($index, (text_ref, indices_ref));
                $top_scores.insert($index, $score);
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
        use std::collections::VecDeque;

        /// The constant to define the length of `top_` queues.
        const ITEMS_TO_SHOW: usize = 7;

        let mut buffer = Vec::with_capacity({
            let (low, high) = iter.size_hint();
            high.unwrap_or(low)
        });

        // VecDeque always leaves one space empty and raw buffer is always a power of two,
        // so 7 capacity will create VecDeque with buffer for 8 items, even though it'll use only 7.
        // Real capacity will be the same for 6 and 5 too, exactly 7.
        let mut top_scores: VecDeque<f64> = VecDeque::with_capacity(ITEMS_TO_SHOW);
        let mut top_results: VecDeque<(&str, &[usize])> = VecDeque::with_capacity(ITEMS_TO_SHOW);

        // First, let's try to produce `ITEMS_TO_SHOW` items to fill the topscores.
        let mut count = 0;
        let if_ok_return = iter.try_for_each(|(text, score, indices)| {
            // Best results are stored in front.
            //XXX I can't say, if bigger score is better or not. Let's assume the bigger the better.
            let idx = match top_scores
                .iter()
                .rev() // .rev(), because worse items are at the end.
                .enumerate()
                .find(|&(_, &other_score)| other_score > score)
            {
                Some((idx, _)) => idx,
                None => top_scores.len(),
            };
            insert_both!(idx, score, text, indices => buffer, top_results, top_scores);

            // Stop iterating after 7 iterations.
            count += 1;
            if count == ITEMS_TO_SHOW {
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
        let mut counter = 0_usize;
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

                counter = counter.wrapping_add(1);
                if counter % 16 == 0 {
                    use std::time::{Duration, Instant};

                    const UPDATE_INTERVAL: Duration = Duration::from_secs(2);

                    let now = Instant::now();
                    if now > past + UPDATE_INTERVAL {
                        past = now;
                        println_json!(top_results);
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
    // I think, it's just good enough. And should be more effective then full
    // `collect()` into Vec on big numbers of iterations.
    fn dyn_collect_number(
        mut iter: impl Iterator<Item = FuzzyMatchedLineInfo>,
        number: usize,
    ) -> (usize, Vec<FuzzyMatchedLineInfo>) {
        use std::collections::VecDeque;

        /// The constant to define the length of `top_` queues.
        const ITEMS_TO_SHOW: usize = 7;

        // To not have problems with queues after sorting and truncating the buffer,
        // buffer has the lowest bound of `ITEMS_TO_SHOW * 2`, not `number * 2`.
        let mut buffer = Vec::with_capacity(2 * std::cmp::max(ITEMS_TO_SHOW, number));

        // VecDeque always leaves one space empty and raw buffer is always a power of two,
        // so 7 capacity will create VecDeque with buffer for 8 items, even though it'll use only 7.
        // Real capacity will be the same for 6 and 5 too, exactly 7.
        let mut top_scores: VecDeque<f64> = VecDeque::with_capacity(ITEMS_TO_SHOW);
        let mut top_results: VecDeque<(&str, &[usize])> = VecDeque::with_capacity(ITEMS_TO_SHOW);

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
                Some((idx, _)) => idx,
                None => top_scores.len(),
            };
            insert_both!(idx, score, text, indices => buffer, top_results, top_scores);

            // Stop iterating after 7 iterations.
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
                        println_json!(top_results);
                    }
                }

                if buffer.len() == buffer.capacity() {
                    buffer.par_sort_unstable_by(|(_, v1, _), (_, v2, _)| v2.partial_cmp(&v1).unwrap());

                    let mut scores = VecDeque::with_capacity(ITEMS_TO_SHOW);
                    let mut results = VecDeque::with_capacity(ITEMS_TO_SHOW);
                    for (text, score, indices) in buffer[..ITEMS_TO_SHOW].iter() {
                        let (text_ref, indices_ref) = unsafe {(
                            std::mem::transmute::<&str, &'static str>(text.as_str()),
                            std::mem::transmute::<&[_], &'static [_]>(indices.as_slice()),
                        )};
                        scores.push_back(*score);
                        results.push_back((text_ref, indices_ref));
                    }

                    top_scores = scores;
                    top_results = results;

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
                list.into_iter().filter_map(|line| {
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
                dyn_collect_all(list.into_iter().filter_map(|line| {
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
