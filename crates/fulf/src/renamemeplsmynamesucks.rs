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
    /// What should be searched.
    pub needle: Box<str>,

    /// If it is `false`, fast ASCII search will be used,
    /// unless needle includes non-ASCII chars.
    ///
    /// If it is `true`, any line matched will be converted to
    /// `str` with [`String::from_utf8_lossy`].
    ///
    /// [`String::from_utf8_lossy`]: https://doc.rust-lang.org/std/string/struct.String.html#method.from_utf8_lossy
    pub force_utf8: bool,

    /// Maximum number of matched and fuzzed results that will remain in memory.
    pub results_cap: usize,

    /// Maximum number of bytes for any line.
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
    pub max_line_len: usize,

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

impl PartialEq for Rules {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.results_cap == other.results_cap
            && self.max_line_len == other.max_line_len
            // && self.with_errors == other.with_errors
            && self.needle == other.needle
            && (self.force_utf8 == other.force_utf8 || !self.needle.is_ascii())
    }
}

impl Default for Rules {
    #[inline]
    fn default() -> Self {
        Self {
            needle: Default::default(),
            force_utf8: false,
            results_cap: 500_usize.next_power_of_two(),
            max_line_len: 1024_usize.next_power_of_two(),
            // with_errors: false,
            bonus_threads: 0,
        }
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
    (ascii, $needle:expr, $max_line_len:expr) => {{
        let max_line_len = $max_line_len;
        let needle: Box<[u8]> = $needle.into();

        let match_and_score = move |line: &[u8]| -> Option<MWP> {
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

    (utf8, $needle:expr, $max_line_len:expr) => {{
        let (needle, max_line_len) = ($needle, $max_line_len);

        let (needle, needle_charcount) = NeedleUTF8::new(needle).unwrap_or_else(Default::default);
        let match_and_score = move |line: &[u8]| -> Option<MWP> {
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

#[inline]
pub fn very_simple(
    path: impl AsRef<Path>,
    needle: impl Into<Box<str>>,
    sort_and_print: impl FnMut(&mut [MWP], usize),
) -> (Vec<MWP>, usize) {
    let r = {
        let mut r = Rules::default();
        r.needle = needle.into();
        r.bonus_threads = if cfg!(target_pointer_width = "64") {
            2
        } else {
            0
        };

        r
    };

    let dir_iter = ignore::Walk::new(path);

    setter(dir_iter, r, sort_and_print)
}

#[inline]
pub fn setter(
    iter: ignore::Walk,
    r: Rules,
    sort_and_print: impl FnMut(&mut [MWP], usize),
) -> (Vec<MWP>, usize) {
    let needle = r.needle;
    let max_line_len = r.max_line_len;

    if needle.is_empty() || needle.len() > max_line_len {
        return Default::default();
    }

    // `spawner()` needs a match and score Fn that returns Option<T>,
    // plus sort and print Fn, that returns nothing but sorts appended results in place.
    //
    // And last, the most tricky part - iterator over iterators that produce a bunch of lines
    // on each iteration (probably just `fs::read(path).unwrap_or(&[])`).
    //
    // First two are no magic, but the last one is tricky. Like, how could it be created?
    //
    // For example, getting a directory to search in, then `ignore::Walk::new()`,
    // then this ignore iterator gets collected into some vectors (one for each thread to spawn),
    // and then that vectors are `into_iter`ed, collected and fed to spawner?

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

    match (r.force_utf8, needle.is_ascii()) {
        // ascii
        (false, true) => {
            let match_and_score = match_and_score!(ascii, needle, max_line_len);

            spawner(files_chunks, capnum, match_and_score, sort_and_print)
        }

        // utf8
        (true, _) | (_, false) => {
            let match_and_score = match_and_score!(utf8, needle, max_line_len);

            spawner(files_chunks, capnum, match_and_score, sort_and_print)
        }
    }
}

#[inline]
fn spawner<T: Send + 'static>(
    files_chunks: impl Iterator<Item = impl Iterator<Item = impl AsRef<[u8]>> + Send + 'static>,
    capnum: usize,
    match_and_score: impl Fn(&[u8]) -> Option<T> + Send + 'static + Clone,
    mut sort_and_print: impl FnMut(&mut [T], usize),
) -> (Vec<T>, usize) {
    let (sx, rx) = flume::unbounded();
    let mut threads = Vec::with_capacity(10);

    files_chunks.for_each(|files| {
        let t;
        let sender = sx.clone();
        let match_and_score = match_and_score.clone();
        t = thread::spawn(move || threadworks::spawn_me(files, sender, capnum, match_and_score));

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

        very_simple(current_dir, "err", sort_and_print);
    }
}
