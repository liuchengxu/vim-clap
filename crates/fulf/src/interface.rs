use {
    crate::{
        ascii::{self, bytes_into_ascii_string_lossy},
        scoring_utils::Score,
        threadworks,
        utf8::{self, NeedleUTF8},
    },
    ignore,
    std::{fs, io, path::Path, thread},
};

pub use threadworks::KillUs;

type ScoringResult = (Box<str>, Score, Box<[usize]>);
type MWP = ScoringResult;

/// A struct to define rules to run fuzzy-search.
///
/// Read fields' documentation for more.
#[derive(Debug, Clone)]
pub struct Rules {
    /// Maximum number of matched and fuzzed results that will remain in memory.
    pub results_cap: usize,

    /// The number of bonus threads to spawn.
    ///
    /// If it is 0, the main thread will be used anyway.
    ///
    /// Fat OS threads are spawned, so there's no point
    /// in any number bigger than `(maximum OS threads) - 1`.
    /// Even worse, any number bigger than this will
    /// decrease performance.
    pub bonus_threads: u8,
}

impl Rules {
    #[inline]
    pub const fn new() -> Self {
        Self {
            results_cap: 512,
            bonus_threads: 0,
        }
    }
}

impl PartialEq for Rules {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.results_cap == other.results_cap
    }
}

impl Default for Rules {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

struct NoError;

impl From<io::Error> for NoError {
    #[inline]
    fn from(_: io::Error) -> Self {
        Self
    }
}

macro_rules! match_and_score {
    (ascii, $max_line_len:expr) => {{
        let max_line_len = $max_line_len;

        let match_and_score = move |line: &[u8], needle: &Box<[u8]>| -> Option<MWP> {
            if line.len() > max_line_len {
                return None;
            }
            let needle = needle.as_ref();

            if let Some(()) = ascii::matcher(line, needle) {
                let (score, posits) = ascii::score_with_positions(needle, line);

                let s = Some((
                    bytes_into_ascii_string_lossy(line.to_owned()).into_boxed_str(),
                    score,
                    posits.into_boxed_slice(),
                ));

                s
            } else {
                None
            }
        };

        match_and_score
    }};

    (utf8, $max_line_len:expr, $needle_charcount:expr) => {{
        let (max_line_len, needle_charcount) = ($max_line_len, $needle_charcount);

        let match_and_score = move |line: &[u8], needle: &NeedleUTF8| -> Option<MWP> {
            if line.len() > max_line_len {
                return None;
            }

            if let Some(()) = utf8::matcher(line, needle.as_matcher_needle()) {
                let (score, posits) = utf8::score_with_positions(
                    needle.as_ref(),
                    needle_charcount,
                    String::from_utf8_lossy(line).as_ref(),
                );
                Some((
                    String::from_utf8_lossy(line).into_owned().into_boxed_str(),
                    score,
                    posits.into_boxed_slice(),
                ))
            } else {
                None
            }
        };

        match_and_score
    }};
}

/// The default search function, very simple to use.
///
/// # Arguments
///
/// `path` - a path of directory to search in.
/// The search respects ignore files and is recursive:
/// all files in the given folder and its subfolders
/// are searched.
///
/// `needle` - a string to fuzzy-search.
///
/// `sort_and_print` - a function or a closure, that takes two arguments:
///
/// 1. A mutable slice of unsorted results provided by fzy algorithm;
/// Those should be always sorted within the function
/// (but partially, as only 512 results are kept in the storage).
///
/// 2. A number of total results that passed the matcher and provided
/// at least some score. The number of total results could be bigger than
/// the length of slice.
///
/// # Returns
///
/// Returns what `spawner` returns, but the type is defined by fzy algorithm.
///
/// # Alternatives
///
/// If you need a better control over algorithms, rules and directory
/// traversal, use `setter` function.
///
/// If you need to read files in a manner different from `ignore::Walk`,
/// you can use `spawner` function.
///
/// If you need something much different than anything there,
/// go and write it yourself.
#[inline]
pub fn default_searcher(
    path: impl AsRef<Path>,
    needle: impl Into<Box<str>>,
    sort_and_print: impl FnMut(&mut [MWP], usize),
) -> (Vec<MWP>, usize) {
    with_fzy_algo(
        path,
        needle,
        1024_usize.next_power_of_two(),
        false,
        sort_and_print,
    )
}

/// `max_line_len` sets maximum number of bytes for any line.
///
/// If the line exceeds that number, it is not checked for match at all.
///
/// # Reasons
///
/// The speed of line-fuzzing is non-linear, thus lines too big
/// can slow down the task significantly. And there's very few reasons
/// for a line to exceed, for example, 1024 bytes:
///
/// 1. This is a line in a text that is not code.
///
/// 2. This is a non-formatted line of automatically generated code.
///
/// 3. This is a very bad code.
///
/// 4. Some very rare other reasons.
///
/// And in any of those cases there's probably no point in fuzzing such line.
///
///
/// If `force_utf8` is `false`, fast ASCII search will be used,
/// unless needle includes non-ASCII chars.
///
/// If it is `true`, any line matched will be converted to
/// `str` with [`String::from_utf8_lossy`].
///
/// [`String::from_utf8_lossy`]: https://doc.rust-lang.org/std/string/struct.String.html#method.from_utf8_lossy
#[inline]
pub fn with_fzy_algo(
    path: impl AsRef<Path>,
    needle: impl Into<Box<str>>,
    max_line_len: usize,
    force_utf8: bool,
    sort_and_print: impl FnMut(&mut [MWP], usize),
) -> (Vec<MWP>, usize) {
    let needle = needle.into();

    if needle.is_empty() || needle.len() > max_line_len {
        return Default::default();
    }

    let r = {
        let mut r = Rules::new();
        r.bonus_threads = if cfg!(target_pointer_width = "64") {
            2
        } else {
            0
        };

        r
    };

    let dir_iter = ignore::Walk::new(path);

    match (force_utf8, needle.is_ascii()) {
        // ascii
        (false, true) => {
            let needle: Box<[u8]> = needle.into();

            let match_and_score = match_and_score!(ascii, max_line_len);

            setter(dir_iter, r, needle, match_and_score, sort_and_print)
        }

        // utf8
        (true, _) | (_, false) => {
            let (needle, needle_charcount) =
                NeedleUTF8::new(needle).unwrap_or_else(Default::default);

            let match_and_score = match_and_score!(utf8, max_line_len, needle_charcount);

            setter(dir_iter, r, needle, match_and_score, sort_and_print)
        }
    }
}

/// A function that turns configured directory iterator
/// and a number of bonus threads into the iterator used by `spawner`.
///
/// Collects all files from iterator into `Rules::bonus_threads + 1`
/// vectors and then passes them as iterators to `spawner` function.
/// `Rules::results_cap` is passed to `spawner` as `capnum`, and
/// all other things are passed as is.
///
/// Returns what `spawner` returns.
#[inline]
pub fn setter<T: Send + 'static, N: Clone + Send + 'static>(
    iter: ignore::Walk,
    r: Rules,
    needle: N,
    match_and_score: impl Fn(&[u8], &N) -> Option<T> + Send + 'static + Copy,
    sort_and_print: impl FnMut(&mut [T], usize),
) -> (Vec<T>, usize) {
    let threadcount = r.bonus_threads + 1;
    let mut files_chunks = vec![Vec::with_capacity(1024); threadcount as usize];

    let usize_tc = threadcount as usize;
    let mut index = 0;
    let mut errcount = 0_u32;
    iter.for_each(|res| match res {
        Ok(dir_entry) => {
            let path = dir_entry.into_path();

            if path.is_file() {
                index += 1;
                if index == usize_tc {
                    index = 0;
                }

                files_chunks[index].push(path);
            }
        }
        Err(_) => {
            errcount += 1;
            if errcount > 16000 {
                panic!()
            }
        }
    });

    let files_chunks = files_chunks.into_iter().map(|vec_with_path| {
        vec_with_path
            .into_iter()
            .filter_map(|path| match fs::read(path) {
                Ok(buffer) => Some(buffer),
                _ => None,
            })
    });

    let capnum = r.results_cap;

    spawner(
        files_chunks,
        capnum,
        needle,
        match_and_score,
        sort_and_print,
    )
}

/// The number of threads to spawn is defined by the number of items
/// in the iterator.
///
/// # Returns
///
/// Vector, already sorted by `sort_and_print` function,
/// and a number of total results
/// (a number of `Some`s provided by `match_and_score` fn).
#[inline]
pub fn spawner<T: Send + 'static, N: Clone + Send + 'static>(
    files_chunks: impl Iterator<Item = impl Iterator<Item = impl AsRef<[u8]>> + Send + 'static>,
    capnum: usize,
    needle: N,
    match_and_score: impl Fn(&[u8], &N) -> Option<T> + Send + 'static + Copy,
    mut sort_and_print: impl FnMut(&mut [T], usize),
) -> (Vec<T>, usize) {
    let (sx, rx) = flume::unbounded();
    let mut threads = Vec::with_capacity(10);

    files_chunks.for_each(|files| {
        let t;
        let sender = sx.clone();
        // let match_and_score = match_and_score.clone();
        let needle = needle.clone();
        t = thread::spawn(move || {
            threadworks::spawn_me(files, sender, capnum, match_and_score, needle)
        });

        threads.push(t);
    });
    drop(sx);

    let mut shared = Vec::with_capacity(capnum * 2);
    let mut total = 0_usize;

    while let Ok(mut inner) = rx.recv() {
        // append_to_shared(&mut shared, &mut inner, &mut total, capnum, sort_and_print);
        if !inner.is_empty() {
            let inner_len = inner.len();

            total = total.wrapping_add(inner_len);

            shared.append(&mut inner);
            sort_and_print(&mut shared, total.clone());
            shared.truncate(capnum);
        }
    }

    threads.into_iter().for_each(|t| t.join().unwrap());

    (shared, total)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, SystemTime};

    #[test]
    fn basic_functionality_test() {
        const DELAY: Duration = Duration::from_secs(2);
        let mut past = SystemTime::now();

        let sort_and_print = |results: &mut [MWP], total| {
            results.sort_unstable_by(|a, b| b.1.cmp(&a.1));

            let now = SystemTime::now();

            if let Ok(dur) = now.duration_since(past) {
                if dur > DELAY {
                    past = now;

                    for idx in 0..8 {
                        if let Some(pack) = results.get(idx) {
                            let (s, _score, pos) = pack;
                            println!("Total: {}\n{}\n{:?}", total, s, pos);
                        } else {
                            break;
                        }
                    }
                }
            }
        };

        let current_dir = std::env::current_dir().unwrap();
        let needle = "sopa";

        let (results, total) = default_searcher(current_dir.clone(), needle, sort_and_print);

        println!("Total: {}\nCapped results: {:?}", total, results);

        let sort_and_print = |results: &mut [MWP], total| {
            results.sort_unstable_by(|a, b| b.1.cmp(&a.1));

            let now = SystemTime::now();

            if let Ok(dur) = now.duration_since(past) {
                if dur > DELAY {
                    past = now;

                    for idx in 0..8 {
                        if let Some(pack) = results.get(idx) {
                            let (s, _score, pos) = pack;
                            println!("Total: {}\n{}\n{:?}", total, s, pos);
                        } else {
                            break;
                        }
                    }
                }
            }
        };
        println!(
            "{:?}",
            with_fzy_algo(current_dir, needle, 1024, true, sort_and_print)
        );
    }
}
