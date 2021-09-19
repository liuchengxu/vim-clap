use std::io::BufRead;
use std::time::{Duration, Instant};

use rayon::slice::ParallelSliceMut;

use icon::{Icon, ICON_LEN};
use matcher::Bonus;
use utility::{println_json, println_json_with_length};

use super::*;
use crate::FilteredItem;

/// The constant to define the length of `top_` queues.
const ITEMS_TO_SHOW: usize = 40;

const MAX_IDX: usize = ITEMS_TO_SHOW - 1;

/// Refresh the top filtered results per 300 ms.
const UPDATE_INTERVAL: Duration = Duration::from_millis(300);

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
    (pop; $index:expr, $score:expr, $item:expr => $buffer:expr, $top_results:expr, $top_scores:expr) => {{
        match $index {
            // If index is last possible, then the worst item is better than this we want to push in,
            // and we do nothing.
            Some(MAX_IDX) => $buffer.push($item),
            // Else, one item gets popped from the queue
            // and other is inserted.
            Some(idx) => {
                insert_both!(idx + 1, $score, $item => $buffer, $top_results, $top_scores);
            }
            None => {
                insert_both!(0, $score, $item => $buffer, $top_results, $top_scores);
            }
        }
    }};

    // This macro pushes all things into buffer and inserts all things into
    // `top_` queues.
    ($index:expr, $score:expr, $item:expr => $buffer:expr, $top_results:expr, $top_scores:expr) => {{
        $buffer.push($item);
        $top_results.pop_and_insert($index, $buffer.len() - 1);
        $top_scores.pop_and_insert($index, $score);
    }};
}

/// Type of matcher scoring.
type Score = i64;

type SelectedTopItemsInfo = (usize, [Score; ITEMS_TO_SHOW], [usize; ITEMS_TO_SHOW]);

/// Returns Ok if all items in the iterator has been processed.
///
/// First, let's try to produce `ITEMS_TO_SHOW` items to fill the topscores.
fn select_top_items_to_show(
    buffer: &mut Vec<FilteredItem>,
    iter: &mut impl Iterator<Item = FilteredItem>,
) -> std::result::Result<usize, SelectedTopItemsInfo> {
    let mut top_scores: [Score; ITEMS_TO_SHOW] = [Score::min_value(); ITEMS_TO_SHOW];
    let mut top_results: [usize; ITEMS_TO_SHOW] = [usize::min_value(); ITEMS_TO_SHOW];

    let mut total = 0;
    let res = iter.try_for_each(|filtered_item| {
        let score = filtered_item.score;
        let idx = match find_best_score_idx(&top_scores, score) {
            Some(idx) => idx + 1,
            None => 0,
        };

        insert_both!(idx, score, filtered_item => buffer, top_results, top_scores);

        // Stop iterating after `ITEMS_TO_SHOW` iterations.
        total += 1;
        if total == ITEMS_TO_SHOW {
            Err(())
        } else {
            Ok(())
        }
    });

    if res.is_ok() {
        Ok(total)
    } else {
        Err((total, top_scores, top_results))
    }
}

/// Returns the index of best score in `top_scores`.
///
/// Best results are stored in front, the bigger the better.
#[inline]
fn find_best_score_idx(top_scores: &[Score; ITEMS_TO_SHOW], score: Score) -> Option<usize> {
    top_scores
        .iter()
        .enumerate()
        .rev() // .rev(), because worse items are at the end.
        .find(|&(_, &other_score)| other_score > score)
        .map(|(idx, _)| idx)
}

/// Watch and send the dynamic filtering progress when neccessary.
#[derive(Clone, Debug)]
pub struct Watcher {
    /// Time of last notification.
    past: Instant,
    /// Number of total matched items.
    total: usize,
    /// Icon.
    icon: Icon,
    /// Lines we sent last time.
    last_lines: Vec<String>,
}

impl Watcher {
    pub fn new(initial_total: usize, icon: Icon) -> Self {
        Self {
            past: Instant::now(),
            total: initial_total,
            icon,
            last_lines: Vec::with_capacity(ITEMS_TO_SHOW),
        }
    }

    /// Send the current best results periodically.
    ///
    /// # NOTE
    ///
    /// Printing to stdout is to send the content to the client.
    pub fn maybe_notify(&mut self, top_results: &[usize; ITEMS_TO_SHOW], buffer: &[FilteredItem]) {
        if self.total % 16 == 0 {
            let now = Instant::now();
            if now > self.past + UPDATE_INTERVAL {
                let mut indices = Vec::with_capacity(ITEMS_TO_SHOW);
                let mut lines = Vec::with_capacity(ITEMS_TO_SHOW);
                for &idx in top_results.iter() {
                    let filtered_item = std::ops::Index::index(buffer, idx);
                    let text = if let Some(ref painter) = self.icon.painter() {
                        indices.push(filtered_item.shifted_indices(ICON_LEN));
                        painter.paint(filtered_item.display_text())
                    } else {
                        indices.push(filtered_item.match_indices.clone());
                        filtered_item.display_text().to_owned()
                    };
                    lines.push(text);
                }

                let total = self.total;

                #[allow(non_upper_case_globals)]
                const method: &str = "s:process_filter_message";
                if self.last_lines != lines.as_slice() {
                    println_json_with_length!(total, lines, indices, method);
                    self.past = now;
                    self.last_lines = lines;
                } else {
                    self.past = now;
                    println_json_with_length!(total, method);
                }
            }
        }
    }
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
fn dyn_collect_all(mut iter: impl Iterator<Item = FilteredItem>, icon: Icon) -> Vec<FilteredItem> {
    let mut buffer = Vec::with_capacity({
        let (low, high) = iter.size_hint();
        high.unwrap_or(low)
    });

    let top_selected_result = select_top_items_to_show(&mut buffer, &mut iter);

    let (total, mut top_scores, mut top_results) = match top_selected_result {
        Ok(_) => return buffer,
        Err((t, top_scores, top_results)) => (t, top_scores, top_results),
    };

    let mut watcher = Watcher::new(total, icon);

    // Now we have the full queue and can just pair `.pop_back()` with `.insert()` to keep
    // the queue with best results the same size.
    iter.for_each(|item| {
        let score = item.score;

        let idx = find_best_score_idx(&top_scores, score);

        insert_both!(pop; idx, score, item => buffer, top_results, top_scores);

        watcher.total += 1;

        watcher.maybe_notify(&top_results, &buffer);
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
    mut iter: impl Iterator<Item = FilteredItem>,
    number: usize,
    icon: Icon,
) -> (usize, Vec<FilteredItem>) {
    // To not have problems with queues after sorting and truncating the buffer,
    // buffer has the lowest bound of `ITEMS_TO_SHOW * 2`, not `number * 2`.
    let mut buffer = Vec::with_capacity(2 * std::cmp::max(ITEMS_TO_SHOW, number));

    let top_selected_result = select_top_items_to_show(&mut buffer, &mut iter);

    let (total, mut top_scores, mut top_results) = match top_selected_result {
        Ok(t) => return (t, buffer),
        Err((t, top_scores, top_results)) => (t, top_scores, top_results),
    };

    let mut watcher = Watcher::new(total, icon);

    // Now we have the full queue and can just pair `.pop_back()` with
    // `.insert()` to keep the queue with best results the same size.
    iter.for_each(|filtered_item| {
        let score = filtered_item.score;
        let idx = find_best_score_idx(&top_scores, score);

        insert_both!(pop; idx, score, filtered_item => buffer, top_results, top_scores);

        watcher.total += 1;

        watcher.maybe_notify(&top_results, &buffer);

        if buffer.len() == buffer.capacity() {
            buffer.par_sort_unstable_by(|v1, v2| v2.score.partial_cmp(&v1.score).unwrap());

            for (idx, FilteredItem { score, .. }) in buffer[..ITEMS_TO_SHOW].iter().enumerate() {
                top_scores[idx] = *score;
                top_results[idx] = idx;
            }

            let half = buffer.len() / 2;
            buffer.truncate(half);
        }
    });

    (watcher.total, buffer)
}

/// Returns the ranked results after applying fuzzy filter given the query string and a list of candidates.
pub fn dyn_run<I: Iterator<Item = SourceItem>>(
    query: &str,
    source: Source<I>,
    FilterContext {
        algo,
        icon,
        number,
        winwidth,
        match_type,
    }: FilterContext,
    bonuses: Vec<Bonus>,
) -> Result<()> {
    let scoring_matcher = matcher::Matcher::with_bonuses(algo, match_type, bonuses);
    let query: Query = query.into();
    let scorer = |item: &SourceItem| scoring_matcher.match_query(item, &query);
    if let Some(number) = number {
        let (total, filtered) = match source {
            Source::Stdin => dyn_collect_number(source_iter_stdin!(scorer), number, icon),
            #[cfg(feature = "enable_dyn")]
            Source::Exec(exec) => dyn_collect_number(source_iter_exec!(scorer, exec), number, icon),
            Source::File(fpath) => {
                dyn_collect_number(source_iter_file!(scorer, fpath), number, icon)
            }
            Source::List(list) => dyn_collect_number(source_iter_list!(scorer, list), number, icon),
        };

        let ranked = sort_initial_filtered(filtered);

        printer::print_dyn_filter_results(ranked, total, number, winwidth.unwrap_or(100), icon);
    } else {
        let filtered = match source {
            Source::Stdin => dyn_collect_all(source_iter_stdin!(scorer), icon),
            #[cfg(feature = "enable_dyn")]
            Source::Exec(exec) => dyn_collect_all(source_iter_exec!(scorer, exec), icon),
            Source::File(fpath) => dyn_collect_all(source_iter_file!(scorer, fpath), icon),
            Source::List(list) => dyn_collect_all(source_iter_list!(scorer, list), icon),
        };

        let ranked = sort_initial_filtered(filtered);

        for FilteredItem {
            source_item,
            match_indices,
            display_text,
            ..
        } in ranked.into_iter()
        {
            let text = display_text.unwrap_or_else(|| source_item.display_text().to_owned());
            let indices = match_indices;
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
        dyn_run(
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

                    String::from_utf8(changing_text.as_ref().to_owned())
                        .unwrap()
                        .into()
                })
                .take(usize::max_value() >> 8),
            ),
            FilterContext::default().number(Some(100)),
            vec![Bonus::None],
        )
        .unwrap()
    }
}
