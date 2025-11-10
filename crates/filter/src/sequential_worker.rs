//! Convert the source item stream to an iterator and run the filtering sequentially.

use crate::{to_clap_item, FilterContext, MatchedItems, SequentialSource};
use icon::Icon;
use printer::{println_json, println_json_with_length, DisplayLines, Printer};
use rayon::slice::ParallelSliceMut;
use std::io::BufRead;
use std::sync::Arc;
use std::time::{Duration, Instant};
use types::{ClapItem, MatchedItem, Query, Rank};

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
    (pop; $index:expr, $score:expr, $item:expr => $buffer:expr, $top_results:expr, $top_ranks:expr) => {{
        match $index {
            // If index is last possible, then the worst item is better than this we want to push in,
            // and we do nothing.
            Some(MAX_IDX) => $buffer.push($item),
            // Else, one item gets popped from the queue
            // and other is inserted.
            Some(idx) => {
                insert_both!(idx + 1, $score, $item => $buffer, $top_results, $top_ranks);
            }
            None => {
                insert_both!(0, $score, $item => $buffer, $top_results, $top_ranks);
            }
        }
    }};

    // This macro pushes all things into buffer and inserts all things into
    // `top_` queues.
    ($index:expr, $score:expr, $item:expr => $buffer:expr, $top_results:expr, $top_ranks:expr) => {{
        $buffer.push($item);
        $top_results.pop_and_insert($index, $buffer.len() - 1);
        $top_ranks.pop_and_insert($index, $score);
    }};
}

struct BufferInitializationResult {
    // If all items have been processed.
    finished: bool,
    total: usize,
    top_ranks: [Rank; ITEMS_TO_SHOW],
    top_results: [usize; ITEMS_TO_SHOW],
}

/// First, let's try to produce `ITEMS_TO_SHOW` items to fill the topscores.
fn initialize_buffer(
    buffer: &mut Vec<MatchedItem>,
    iter: &mut impl Iterator<Item = MatchedItem>,
) -> BufferInitializationResult {
    let mut top_ranks: [Rank; ITEMS_TO_SHOW] = [Rank::default(); ITEMS_TO_SHOW];
    let mut top_results: [usize; ITEMS_TO_SHOW] = [usize::MIN; ITEMS_TO_SHOW];

    let mut total = 0;
    let res = iter.try_for_each(|matched_item| {
        let rank = matched_item.rank;
        let idx = match find_best_rank_idx(&top_ranks, rank) {
            Some(idx) => idx + 1,
            None => 0,
        };

        insert_both!(idx, rank, matched_item => buffer, top_results, top_ranks);

        // Stop iterating after `ITEMS_TO_SHOW` iterations.
        total += 1;
        if total == ITEMS_TO_SHOW {
            Err(())
        } else {
            Ok(())
        }
    });

    BufferInitializationResult {
        finished: res.is_ok(),
        total,
        top_ranks,
        top_results,
    }
}

/// Returns the index of best score in `top_ranks`.
///
/// Best results are stored in front, the bigger the better.
#[inline]
fn find_best_rank_idx(top_ranks: &[Rank; ITEMS_TO_SHOW], rank: Rank) -> Option<usize> {
    top_ranks
        .iter()
        .enumerate()
        .rev() // .rev(), because worse items are at the end.
        .find(|&(_, &other_rank)| other_rank > rank)
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

fn decorate_line(matched_item: &MatchedItem, icon: Icon) -> (String, Vec<usize>) {
    if let Some(icon_kind) = icon.icon_kind() {
        (
            icon_kind.add_icon_to_text(matched_item.display_text()),
            matched_item.shifted_indices(icon::ICON_CHAR_LEN),
        )
    } else {
        (
            matched_item.display_text().into(),
            matched_item.indices.clone(),
        )
    }
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
    pub fn try_notify(&mut self, top_results: &[usize; ITEMS_TO_SHOW], buffer: &[MatchedItem]) {
        if self.total.is_multiple_of(16) {
            let now = Instant::now();
            if now > self.past + UPDATE_INTERVAL {
                let mut indices = Vec::with_capacity(ITEMS_TO_SHOW);
                let mut lines = Vec::with_capacity(ITEMS_TO_SHOW);
                for &idx in top_results.iter() {
                    let matched_item = std::ops::Index::index(buffer, idx);
                    let (line, line_indices) = decorate_line(matched_item, self.icon);
                    indices.push(line_indices);
                    lines.push(line);
                }

                let total = self.total;

                #[allow(non_upper_case_globals)]
                const deprecated_method: &str = "clap#legacy#state#process_filter_message";
                if self.last_lines != lines.as_slice() {
                    let icon_added = self.icon.enabled();
                    println_json_with_length!(total, lines, indices, deprecated_method, icon_added);
                    self.past = now;
                    self.last_lines = lines;
                } else {
                    self.past = now;
                    println_json_with_length!(total, deprecated_method);
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
fn dyn_collect_all(mut iter: impl Iterator<Item = MatchedItem>, icon: Icon) -> Vec<MatchedItem> {
    let mut buffer = Vec::with_capacity({
        let (low, high) = iter.size_hint();
        high.unwrap_or(low)
    });

    let BufferInitializationResult {
        finished,
        total,
        mut top_ranks,
        mut top_results,
    } = initialize_buffer(&mut buffer, &mut iter);

    if finished {
        return buffer;
    }

    let mut watcher = Watcher::new(total, icon);

    // Now we have the full queue and can just pair `.pop_back()` with `.insert()` to keep
    // the queue with best results the same size.
    iter.for_each(|item| {
        let rank = item.rank;

        let idx = find_best_rank_idx(&top_ranks, rank);

        insert_both!(pop; idx, rank, item => buffer, top_results, top_ranks);

        watcher.total += 1;

        watcher.try_notify(&top_results, &buffer);
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
    mut iter: impl Iterator<Item = MatchedItem>,
    number: usize,
    icon: Icon,
) -> (usize, Vec<MatchedItem>) {
    // To not have problems with queues after sorting and truncating the buffer,
    // buffer has the lowest bound of `ITEMS_TO_SHOW * 2`, not `number * 2`.
    let mut buffer = Vec::with_capacity(2 * ITEMS_TO_SHOW.max(number));

    let BufferInitializationResult {
        finished,
        total,
        mut top_ranks,
        mut top_results,
    } = initialize_buffer(&mut buffer, &mut iter);

    if finished {
        return (total, buffer);
    }

    let mut watcher = Watcher::new(total, icon);

    // Now we have the full queue and can just pair `.pop_back()` with
    // `.insert()` to keep the queue with best results the same size.
    iter.for_each(|matched_item| {
        let rank = matched_item.rank;
        let idx = find_best_rank_idx(&top_ranks, rank);

        insert_both!(pop; idx, rank, matched_item => buffer, top_results, top_ranks);

        watcher.total += 1;

        watcher.try_notify(&top_results, &buffer);

        if buffer.len() == buffer.capacity() {
            buffer.par_sort_unstable_by(|v1, v2| v2.rank.cmp(&v1.rank));

            for (idx, MatchedItem { rank, .. }) in buffer[..ITEMS_TO_SHOW].iter().enumerate() {
                top_ranks[idx] = *rank;
                top_results[idx] = idx;
            }

            let half = buffer.len() / 2;
            buffer.truncate(half);
        }
    });

    (watcher.total, buffer)
}

fn print_on_dyn_run_finished(display_lines: DisplayLines, total_matched: usize) {
    let DisplayLines {
        lines,
        indices,
        truncated_map,
        icon_added,
    } = display_lines;

    #[allow(non_upper_case_globals)]
    const deprecated_method: &str = "clap#legacy#state#process_filter_message";
    println_json_with_length!(
        deprecated_method,
        lines,
        indices,
        icon_added,
        truncated_map,
        total_matched
    );
}

/// Returns the ranked results after applying fuzzy filter given the query string and a list of candidates.
pub fn dyn_run<I: Iterator<Item = Arc<dyn ClapItem>>>(
    query: &str,
    filter_context: FilterContext,
    source: SequentialSource<I>,
) -> crate::Result<()> {
    let FilterContext {
        icon,
        number,
        winwidth,
        matcher_builder,
    } = filter_context;

    let query: Query = query.into();
    let matcher = matcher_builder.build(query);

    let clap_item_stream: Box<dyn Iterator<Item = Arc<dyn ClapItem>>> = match source {
        SequentialSource::Iterator(list) => Box::new(list),
        SequentialSource::Stdin => Box::new(
            std::io::stdin()
                .lock()
                .lines()
                .map_while(Result::ok)
                .filter_map(|line| to_clap_item(matcher.match_scope(), line)),
        ),
        SequentialSource::File(path) => Box::new(
            std::io::BufReader::new(std::fs::File::open(path)?)
                .lines()
                .map_while(Result::ok)
                .filter_map(|line| to_clap_item(matcher.match_scope(), line)),
        ),
        SequentialSource::Exec(exec) => Box::new(
            std::io::BufReader::new(exec.stream_stdout()?)
                .lines()
                .map_while(Result::ok)
                .filter_map(|line| to_clap_item(matcher.match_scope(), line)),
        ),
    };

    let matched_item_stream = clap_item_stream.filter_map(|item| matcher.match_item(item));

    if let Some(number) = number {
        let (total_matched, matched_items) = dyn_collect_number(matched_item_stream, number, icon);
        let mut matched_items = MatchedItems::from(matched_items).par_sort().inner();
        matched_items.truncate(number);

        let printer = Printer::new(winwidth.unwrap_or(100), icon);
        let display_lines = printer.to_display_lines(matched_items);
        print_on_dyn_run_finished(display_lines, total_matched);
    } else {
        let matched_items = dyn_collect_all(matched_item_stream, icon);
        let matched_items = MatchedItems::from(matched_items).par_sort().inner();

        matched_items.iter().for_each(|matched_item| {
            let indices = &matched_item.indices;
            let text = matched_item.display_text();
            println_json!(text, indices);
        });
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
            FilterContext::default().number(Some(100)),
            SequentialSource::Iterator(
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
                        println!("Total lines created: {total_lines_created}")
                    }

                    let item: Arc<dyn ClapItem> =
                        Arc::new(String::from_utf8(changing_text.as_ref().to_owned()).unwrap());

                    item
                })
                .take(usize::MAX >> 8),
            ),
        )
        .unwrap()
    }
}
