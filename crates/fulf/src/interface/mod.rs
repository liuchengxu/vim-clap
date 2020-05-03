use {
    crate::{
        ascii::{self, ByteLines},
        scoring_utils::{MatchWithPositions, MWP},
        utf8,
    },
    ignore,
    std::{
        fs, mem,
        path::{Path, MAIN_SEPARATOR},
        thread,
    },
};

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

/// A trait to define algorithm.
pub trait FuzzySearcher: Sized {
    /// The datapack needed for algorithm to work.
    type SearchData: Clone + Send + 'static;

    /// Like `Iterator::Item`.
    type Item: Send + 'static;

    /// A function to create the struct. Akin to `new`.
    fn create(files: Vec<Box<Path>>, search_data: Self::SearchData) -> Self;

    /// Like `Iterator::for_each`, but doesn't need `next` at all, thus much simpler.
    fn for_each<F: FnMut(Self::Item)>(self, f: F);

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
    fn setter(
        iter: ignore::Walk,
        needle: Self::SearchData,

        r: Rules,

        sort_and_print: impl FnMut(&mut [Self::Item], usize),
    ) -> (Vec<Self::Item>, usize) {
        let threadcount = r.bonus_threads + 1;
        let mut files_chunks = vec![Vec::with_capacity(1024); threadcount as usize];

        let usize_tc = threadcount as usize;
        let mut index = 0;
        let mut errcount = 0;
        iter.for_each(|res| match res {
            Ok(dir_entry) => {
                let path = dir_entry.into_path().into_boxed_path();

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

        let files_chunks = files_chunks.into_iter();

        let capnum = r.results_cap;

        Self::spawner(files_chunks, needle, capnum, sort_and_print)
    }

    /// The number of threads to spawn is defined by the number of items
    /// in the iterator.
    ///
    /// # Returns
    ///
    /// Vector, already sorted by `sort_and_print` function,
    /// and a number of total results.
    #[inline]
    fn spawner(
        files_chunks: impl Iterator<Item = Vec<Box<Path>>>,
        needle: Self::SearchData,

        capnum: usize,

        mut sort_and_print: impl FnMut(&mut [Self::Item], usize),
    ) -> (Vec<Self::Item>, usize) {
        let (sx, rx) = flume::bounded(100);
        let mut threads = Vec::with_capacity(16);

        files_chunks.for_each(|files| {
            let t;
            let sender = sx.clone();
            let needle = needle.clone();
            t = thread::spawn(move || spawn_me(Self::create(files, needle), sender, capnum));

            threads.push(t);
        });
        drop(sx);

        let mut shared = Vec::with_capacity(capnum * 2);
        let mut total = 0_usize;

        while let Ok(mut inner) = rx.recv() {
            if !inner.is_empty() {
                let inner_len = inner.len();

                total = total.wrapping_add(inner_len);

                shared.append(&mut inner);
                sort_and_print(&mut shared, total);
                shared.truncate(capnum);
            }
        }

        threads.into_iter().for_each(|t| t.join().unwrap());

        (shared, total)
    }
}

#[inline]
fn spawn_me<FE: FuzzySearcher>(resulter: FE, sender: flume::Sender<Vec<FE::Item>>, capnum: usize) {
    let mut inner = Vec::with_capacity(capnum);

    resulter.for_each(|result| {
        if inner.len() == inner.capacity() {
            let msg = mem::replace(&mut inner, Vec::with_capacity(capnum));

            let _any_result = sender.send(msg);
        }

        inner.push(result);
    });

    // Whatever is is, we will end this function's work right here anyway.
    let _any_result = sender.send(inner);
}

#[derive(Clone)]
pub struct FzyData {
    pub root_folder: Box<str>,
    pub needle: Box<str>,
    pub max_line_len: usize,
}

impl FzyData {
    pub fn new(root_folder: Box<str>, needle: Box<str>, max_line_len: usize) -> Self {
        Self {
            root_folder,
            needle,
            max_line_len,
        }
    }
}

/// Default algorithm.
pub struct FzyAscii {
    files: Vec<Box<Path>>,
    root_folder: Box<str>,
    needle: Box<str>,
    max_line_len: usize,
}

impl FuzzySearcher for FzyAscii {
    type SearchData = FzyData;

    type Item = MWP;

    #[inline]
    fn create(files: Vec<Box<Path>>, search_data: Self::SearchData) -> Self {
        let FzyData {
            root_folder,
            needle,
            max_line_len,
        } = search_data;

        Self {
            files,
            root_folder,
            needle,
            max_line_len,
        }
    }

    #[inline]
    fn for_each<F: FnMut(Self::Item)>(self, mut f: F) {
        let needle = self.needle.as_bytes();

        self.files.iter().for_each(|file| {
            if let Ok(filebuf) = fs::read(file) {
                match ascii::ascii_from_bytes(&filebuf) {
                    // Checked ASCII
                    Some(ascii_str) => {
                        ByteLines::new(ascii_str.as_bytes()).enumerate().for_each(
                            |(line_idx, line)| {
                                if line.len() > self.max_line_len {
                                    return;
                                }

                                if let Some(()) = ascii::matcher(line, needle) {
                                    let (score, pos) = ascii::score_with_positions(needle, line);

                                    let path_with_root = file.as_os_str().to_string_lossy();
                                    let path_with_root = path_with_root.as_ref();

                                    let path_without_root = path_with_root
                                        .get(self.root_folder.len()..)
                                        .map(|path| {
                                            let is_slash = path
                                                .as_bytes()
                                                .get(0)
                                                .map(|byte| *byte == MAIN_SEPARATOR as u8)
                                                .unwrap_or(false);
                                            if is_slash {
                                                &path[1..]
                                            } else {
                                                path
                                            }
                                        })
                                        .unwrap_or(path_with_root);

                                    // SAFETY: the whole text is checked and is ASCII, which is utf8 always;
                                    // the line is a part of a text, so is utf8 too.
                                    let line = unsafe { std::str::from_utf8_unchecked(line) };

                                    f((
                                        format!("{}:{}:1{}", path_without_root, line_idx, line),
                                        score,
                                        pos.into_boxed_slice(),
                                    ))
                                }
                            },
                        );
                    }
                    // Maybe utf8. Fall back to utf8 scoring for as long as it is valid utf8.
                    None => fallback_utf8(
                        file,
                        &filebuf,
                        self.root_folder.as_ref(),
                        &self.needle,
                        self.max_line_len,
                        &mut f,
                    ),
                }
            }
        });
    }
}

fn fallback_utf8<F: FnMut(MWP)>(
    file: &Path,
    filebuf: &[u8],
    root_folder: &str,
    needle: &str,
    max_line_len: usize,
    mut f: F,
) {
    let valid_up_to = match std::str::from_utf8(filebuf) {
        Ok(_valid_str) => filebuf.len(),
        Err(utf8_e) => utf8_e.valid_up_to(),
    };

    // SAFETY: just checked validness.
    let valid_str = unsafe { std::str::from_utf8_unchecked(&filebuf[..valid_up_to]) };

    valid_str.lines().enumerate().for_each(|(line_idx, line)| {
        if line.len() > max_line_len {
            return;
        }

        if let Some((score, pos)) = utf8::match_and_score_with_positions(needle, line) {
            let path_with_root = file.as_os_str().to_string_lossy();
            let path_with_root = path_with_root.as_ref();

            let path_without_root = path_with_root
                .get(root_folder.len()..)
                .map(|path| {
                    let is_slash = path
                        .as_bytes()
                        .get(0)
                        .map(|byte| *byte == MAIN_SEPARATOR as u8)
                        .unwrap_or(false);
                    if is_slash {
                        &path[1..]
                    } else {
                        path
                    }
                })
                .unwrap_or(path_with_root);

            f((
                format!("{}:{}:1{}", path_without_root, line_idx, line),
                score,
                pos.into_boxed_slice(),
            ))
        }
    });
}

pub struct GenericAlgo<A>
where
    A: Fn(&str, &str) -> Option<MatchWithPositions> + Clone + Send + 'static,
{
    files: Vec<Box<Path>>,
    search_data: GenericSearchData<A>,
}

#[derive(Clone)]
pub struct GenericSearchData<A>
where
    A: Fn(&str, &str) -> Option<MatchWithPositions> + Clone + Send + 'static,
{
    root_folder: Box<str>,
    needle: Box<str>,
    algo: A,
}

impl<A> GenericSearchData<A>
where
    A: Fn(&str, &str) -> Option<MatchWithPositions> + Clone + Send + 'static,
{
    pub fn new(root_folder: Box<str>, needle: Box<str>, algo: A) -> Self {
        Self {
            root_folder,
            needle,
            algo,
        }
    }
}

impl<A> FuzzySearcher for GenericAlgo<A>
where
    A: Fn(&str, &str) -> Option<MatchWithPositions> + Clone + Send + 'static,
{
    type SearchData = GenericSearchData<A>;

    type Item = MWP;

    fn create(files: Vec<Box<Path>>, search_data: Self::SearchData) -> Self {
        Self { files, search_data }
    }

    fn for_each<F: FnMut(Self::Item)>(self, mut f: F) {
        let root_folder = self.search_data.root_folder.as_ref();
        let needle = self.search_data.needle.as_ref();
        let algo = self.search_data.algo;

        self.files.iter().for_each(|file| {
            if let Ok(filebuf) = fs::read(file) {
                let valid_up_to = match std::str::from_utf8(&filebuf) {
                    Ok(_valid_str) => filebuf.len(),
                    Err(utf8_e) => utf8_e.valid_up_to(),
                };

                // SAFETY: just checked validness.
                let valid_str = unsafe { std::str::from_utf8_unchecked(&filebuf[..valid_up_to]) };

                valid_str.lines().enumerate().for_each(|(line_idx, line)| {
                    if let Some((score, pos)) = algo(needle, line) {
                        let path_with_root = file.as_os_str().to_string_lossy();
                        let path_with_root = path_with_root.as_ref();

                        let path_without_root = path_with_root
                            .get(root_folder.len()..)
                            .map(|path| {
                                let is_slash = path
                                    .as_bytes()
                                    .get(0)
                                    .map(|byte| *byte == MAIN_SEPARATOR as u8)
                                    .unwrap_or(false);
                                if is_slash {
                                    &path[1..]
                                } else {
                                    path
                                }
                            })
                            .unwrap_or(path_with_root);

                        f((
                            format!("{}:{}:1{}", path_without_root, line_idx, line),
                            score,
                            pos.into_boxed_slice(),
                        ))
                    }
                });
            }
        });
    }
}

/// More of an example, than real thing, yeah. But could be useful.
#[cfg(test)]
mod showcase {
    use super::*;

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
    ) -> Option<(Vec<MWP>, usize)> {
        with_fzy_algo(path, needle, 1024_usize.next_power_of_two(), sort_and_print)
    }

    /// A function to use default fuzzy-search algorithm.
    ///
    /// # Returns
    ///
    /// Return `None` if the root path cannot be represented as a utf8.
    ///
    /// # Maximum line length
    ///
    /// `max_line_len` sets maximum number of bytes for any line.
    ///
    /// If the line exceeds that number, it is not checked for match at all.
    ///
    /// Reasons:
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
    /// 4. Some very rare other reasons, like giant right-shifted branching.
    ///
    /// And in any of those cases there's probably no point in fuzzing such line.
    #[inline]
    pub fn with_fzy_algo(
        path: impl AsRef<Path>,

        needle: impl Into<Box<str>>,
        max_line_len: usize,

        sort_and_print: impl FnMut(&mut [MWP], usize),
    ) -> Option<(Vec<MWP>, usize)> {
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

        let path = path.as_ref();
        let dir_iter = ignore::Walk::new(path);

        let root_folder = path.to_str()?;

        let is_ascii = needle.is_ascii();

        Some(if is_ascii {
            // ascii
            let fzydata = FzyData::new(root_folder.into(), needle, max_line_len);
            FzyAscii::setter(dir_iter, fzydata, r, sort_and_print)
        } else {
            // utf8
            let algo = move |needle: &str, line: &str| {
                if line.len() > max_line_len {
                    None
                } else {
                    utf8::match_and_score_with_positions(needle, line)
                }
            };

            let data = GenericSearchData::new(root_folder.into(), needle, algo);
            GenericAlgo::setter(dir_iter, data, r, sort_and_print)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{showcase::*, *};
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

        let (results, total) =
            default_searcher(current_dir.clone(), needle, sort_and_print).unwrap();

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
            with_fzy_algo(current_dir, needle, 1024, sort_and_print)
        );
    }
}
