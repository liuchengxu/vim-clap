use {
    crate::{
        ascii::{self, ByteLines},
        scoring_utils::{MatchWithPositions, Score, MWP},
    },
    ignore,
    std::{
        fs, mem,
        path::{Path, MAIN_SEPARATOR},
        sync::Arc,
        thread,
    },
};

/// A struct to define rules to run fuzzy-search.
///
/// Read fields' documentation for more.
#[derive(Debug, Clone)]
pub struct Rules {
    /// Maximum number of matched and fuzzed results
    /// that will remain in memory of every spawned thread
    /// until passed down to the synchronization function.
    pub thread_local_results_cap: usize,

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
            thread_local_results_cap: 128,
            bonus_threads: 0,
        }
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

        sort_and_print: impl FnMut(Vec<Self::Item>),
    ) {
        let threadcount = r.bonus_threads as usize + 1;
        let mut files_chunks = vec![Vec::with_capacity(1024); threadcount];

        let mut index = 0;
        let mut errcount = 0;
        iter.for_each(|res| match res {
            Ok(dir_entry) => {
                let path = dir_entry.into_path().into_boxed_path();

                if path.is_file() {
                    index += 1;
                    if index == threadcount {
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

        Self::spawner(
            files_chunks,
            needle,
            r.thread_local_results_cap,
            sort_and_print,
        );
    }

    /// The number of threads to spawn is defined by the number of items
    /// in the iterator.
    #[inline]
    fn spawner(
        files_chunks: impl Iterator<Item = Vec<Box<Path>>>,
        needle: Self::SearchData,

        capnum: usize,

        sort_and_print: impl FnMut(Vec<Self::Item>),
    ) {
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

        rx.iter().for_each(sort_and_print);

        threads.into_iter().for_each(|t| t.join().unwrap());
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

    // The last vector could be empty or partially filled.
    if !inner.is_empty() {
        // Whatever is is, we will end this function's work right here anyway.
        let _any_result = sender.send(inner);
    }
}

pub struct AsciiAlgo<A, S>
where
    A: Fn(&[u8], &[u8], &mut (Vec<Score>, Vec<Score>)) -> Option<MatchWithPositions>
        + Clone
        + Send
        + 'static,
    S: Fn(&str, &str, &mut (Vec<Score>, Vec<Score>)) -> Option<MatchWithPositions>
        + Clone
        + Send
        + 'static,
{
    files: Vec<Box<Path>>,
    search_data: AsciiSearchData<A, S>,
}

#[derive(Clone)]
pub struct AsciiSearchData<A, S>
where
    A: Fn(&[u8], &[u8], &mut (Vec<Score>, Vec<Score>)) -> Option<MatchWithPositions>
        + Clone
        + Send
        + 'static,
    S: Fn(&str, &str, &mut (Vec<Score>, Vec<Score>)) -> Option<MatchWithPositions>
        + Clone
        + Send
        + 'static,
{
    root_folder: Arc<str>,
    needle: Arc<str>,
    algo: A,
    fallback_utf8_algo: S,
}

impl<A, S> AsciiSearchData<A, S>
where
    A: Fn(&[u8], &[u8], &mut (Vec<Score>, Vec<Score>)) -> Option<MatchWithPositions>
        + Clone
        + Send
        + 'static,
    S: Fn(&str, &str, &mut (Vec<Score>, Vec<Score>)) -> Option<MatchWithPositions>
        + Clone
        + Send
        + 'static,
{
    pub fn new(root_folder: Arc<str>, needle: Arc<str>, algo: A, fallback_utf8_algo: S) -> Self {
        Self {
            root_folder,
            needle,
            algo,
            fallback_utf8_algo,
        }
    }
}

impl<A, S> FuzzySearcher for AsciiAlgo<A, S>
where
    A: Fn(&[u8], &[u8], &mut (Vec<Score>, Vec<Score>)) -> Option<MatchWithPositions>
        + Clone
        + Send
        + 'static,
    S: Fn(&str, &str, &mut (Vec<Score>, Vec<Score>)) -> Option<MatchWithPositions>
        + Clone
        + Send
        + 'static,
{
    type SearchData = AsciiSearchData<A, S>;

    type Item = MWP;

    #[inline]
    fn create(files: Vec<Box<Path>>, search_data: Self::SearchData) -> Self {
        Self { files, search_data }
    }

    #[inline]
    fn for_each<F: FnMut(Self::Item)>(self, mut f: F) {
        fn trim_ascii_whitespace(line: &str) -> (&str, usize) {
            let mut iter = line.as_bytes().iter().enumerate();

            let start_idx = iter
                .find(|(_idx, c)| !c.is_ascii_whitespace())
                .map(|idx_c| idx_c.0)
                // This trim should not be used on an empty line,
                // but if it would, the line will be indexed with the
                // [0..0] range and won't panic.
                .unwrap_or(0);

            let end_idx = iter
                .rfind(|(_idx, c)| !c.is_ascii_whitespace())
                //x Inclusive range could not be used;
                //x even though `[1..=0]` won't panic,
                //x on a string that has only whitespaces,
                //x the range will be [0..=0], which is not okay.
                //
                // `+1` because current index is the index of a
                // first non-whitespace char, but range is not inclusive.
                .map(|idx_c| idx_c.0 + 1)
                .unwrap_or(start_idx);

            // Because the index starts from 0
            // and there's only one byte for each ASCII char,
            // the number of trimmed whitespaces is `start_idx`.
            (&line[start_idx..end_idx], start_idx)
        }

        let needle: &str = &self.search_data.needle;
        let root_folder: &str = &self.search_data.root_folder;

        let algo: &A = &self.search_data.algo;

        let fallback_algo: &S = &self.search_data.fallback_utf8_algo;

        let mut prealloc: (Vec<Score>, Vec<Score>) = (Vec::new(), Vec::new());

        self.files.iter().for_each(|file| {
            if let Ok(filebuf) = fs::read(file) {
                match ascii::ascii_from_bytes(&filebuf) {
                    // Checked ASCII
                    Some(ascii_str) => {
                        ByteLines::new(ascii_str.as_bytes()).enumerate().for_each(
                            |(line_idx, line)| {
                                // SAFETY: the whole text is checked and is ASCII, which is utf8 always;
                                // the line is a part of a text, so is utf8 too.
                                let line = unsafe { std::str::from_utf8_unchecked(line) };

                                let ascii_algo = |line: &str| {
                                    algo(line.as_bytes(), needle.as_bytes(), &mut prealloc)
                                };

                                apply(
                                    trim_ascii_whitespace,
                                    ascii_algo,
                                    line,
                                    file,
                                    root_folder,
                                    line_idx,
                                    &mut f,
                                );
                            },
                        );
                    }
                    // Maybe utf8. Fall back to utf8 scoring for as long as it is valid utf8.
                    None => {
                        let utf8_line = |line: &str| fallback_algo(line, needle, &mut prealloc);

                        generic_utf8(file, &filebuf, root_folder, utf8_line, &mut f)
                    }
                }
            }
        });
    }
}

fn generic_utf8<F: FnMut(MWP)>(
    file: &Path,
    filebuf: &[u8],
    root_folder: &str,
    mut takes_line: impl FnMut(&str) -> Option<MatchWithPositions>,
    mut f: F,
) {
    fn trim_utf8_whitespace(line: &str) -> (&str, usize) {
        let mut trimmed_start: usize = 0;
        let line = line.trim_start_matches(|c: char| {
            let is_w = c.is_whitespace();
            if is_w {
                trimmed_start += 1;
            }

            is_w
        });

        (line.trim_end(), trimmed_start)
    }

    let valid_up_to = match std::str::from_utf8(filebuf) {
        Ok(_valid_str) => filebuf.len(),
        Err(utf8_e) => utf8_e.valid_up_to(),
    };

    // SAFETY: just checked validness.
    let valid_str = unsafe { std::str::from_utf8_unchecked(&filebuf[..valid_up_to]) };

    valid_str.lines().enumerate().for_each(|(line_idx, line)| {
        apply(
            trim_utf8_whitespace,
            &mut takes_line,
            line,
            file,
            root_folder,
            line_idx,
            &mut f,
        );
    });
}

#[allow(clippy::too_many_arguments)]
fn apply(
    // ASCII trimming gets some bonuses,
    // so this is not generic over utf8.
    //
    // Should return the trimmed string,
    // and the number of chars trimmed from the start,
    // because that number is added to the column.
    trim_whitespaces: impl Fn(&str) -> (&str, usize),
    mut takes_line: impl FnMut(&str) -> Option<MatchWithPositions>,
    line: &str,
    filepath: &Path,
    root_folder: &str,
    line_idx: usize,
    mut f: impl FnMut(MWP),
) {
    if let Some((score, pos)) = takes_line(line) {
        let path_with_root = filepath.as_os_str().to_string_lossy();
        let path_with_root = path_with_root.as_ref();

        let path_without_root = path_with_root
            .get(root_folder.len()..)
            .map(|path| {
                path.chars()
                    .next()
                    .map(|ch| {
                        if ch == MAIN_SEPARATOR {
                            let mut buf = [0_u8; 4];
                            let sep_len = ch.encode_utf8(&mut buf).len();

                            &path[sep_len..]
                        } else {
                            path
                        }
                    })
                    .unwrap_or(path)
            })
            .unwrap_or(path_with_root);

        // N.B. Cannot trim before the algorithm,
        // because this could change the result
        // (trailing or leading whitespaces are valid to search,
        // even if that's a very rare case).
        let (trimmed_line, add_col) = trim_whitespaces(line);
        let bufs = (&mut [0_u8; 20], &mut [0_u8; 20]);
        // Humans' numbers start from 1.
        let row = fmt_usize(1 + line_idx, bufs.0);
        let col = fmt_usize(1 + add_col, bufs.1);
        // Three `:` chars, plus all other chars;
        // `row` and `len` are ascii digits, thus `len()`, not `chars().count()`.
        let path_row_col_len = 3 + path_without_root.chars().count() + row.len() + col.len();
        let mut pos = pos;
        pos.iter_mut().for_each(|p| {
            // Move right by the length of things before the line.
            *p += path_row_col_len;
            // Move left by the number of trimmed whitespace chars.
            *p -= add_col;
        });

        f((
            format!(
                "{}:{row}:{col}:{line}",
                path_without_root,
                row = row,
                col = col,
                line = trimmed_line,
            ),
            score,
            pos.into_boxed_slice(),
        ))
    }
}

/// Formats the number, returns the string.
///
/// Could be used with stack-allocated buffer.
///
/// # Panic
///
/// Panics if the buffer is not big enough.
///
/// # Note
///
/// As long as `usize` is not wider than u64,
/// a buffer with 20 bytes is enough.
fn fmt_usize(u: usize, buf: &mut [u8]) -> &mut str {
    let len = buf.len();

    let mut index = len;
    let mut u = u;
    while u != 0 {
        index -= 1;
        buf[index] = (u % 10) as u8 + b'0';
        u /= 10;
    }

    // SAFETY: "mod 10 + b'0'" gives only ASCII chars, which is always utf8.
    unsafe { std::str::from_utf8_unchecked_mut(&mut buf[index..len]) }
}

pub struct Utf8Algo<A>
where
    A: Fn(&str, &str, &mut (Vec<Score>, Vec<Score>)) -> Option<MatchWithPositions>
        + Clone
        + Send
        + 'static,
{
    files: Vec<Box<Path>>,
    search_data: Utf8SearchData<A>,
}

#[derive(Clone)]
pub struct Utf8SearchData<A>
where
    A: Fn(&str, &str, &mut (Vec<Score>, Vec<Score>)) -> Option<MatchWithPositions>
        + Clone
        + Send
        + 'static,
{
    root_folder: Arc<str>,
    needle: Arc<str>,
    algo: A,
}

impl<A> Utf8SearchData<A>
where
    A: Fn(&str, &str, &mut (Vec<Score>, Vec<Score>)) -> Option<MatchWithPositions>
        + Clone
        + Send
        + 'static,
{
    pub fn new(root_folder: Arc<str>, needle: Arc<str>, algo: A) -> Self {
        Self {
            root_folder,
            needle,
            algo,
        }
    }
}

impl<A> FuzzySearcher for Utf8Algo<A>
where
    A: Fn(&str, &str, &mut (Vec<Score>, Vec<Score>)) -> Option<MatchWithPositions>
        + Clone
        + Send
        + 'static,
{
    type SearchData = Utf8SearchData<A>;

    type Item = MWP;

    fn create(files: Vec<Box<Path>>, search_data: Self::SearchData) -> Self {
        Self { files, search_data }
    }

    fn for_each<F: FnMut(Self::Item)>(self, mut f: F) {
        let root_folder: &str = &self.search_data.root_folder;
        let needle: &str = &self.search_data.needle;
        let algo: &A = &self.search_data.algo;

        let mut prealloc: (Vec<Score>, Vec<Score>) = (Vec::new(), Vec::new());

        self.files.iter().for_each(|file| {
            if let Ok(filebuf) = fs::read(file) {
                let takes_line = |line: &str| algo(line, needle, &mut prealloc);

                generic_utf8(file, &filebuf, root_folder, takes_line, &mut f);
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
    /// Returns what `spawner` returns.
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
        needle: impl AsRef<str>,
        sort_and_print: impl FnMut(Vec<MWP>),
    ) -> Result<(), ()> {
        with_fzy_algo(path, needle, 1024_usize.next_power_of_two(), sort_and_print)
    }

    /// A function to use default fuzzy-search algorithm.
    ///
    /// # Returns
    ///
    /// Return `Err` if the root path cannot be represented as a utf8.
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

        needle: impl AsRef<str>,
        max_line_len: usize,

        sort_and_print: impl FnMut(Vec<MWP>),
    ) -> Result<(), ()> {
        let needle = needle.as_ref();

        if needle.is_empty() || needle.len() > max_line_len {
            return Err(());
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

        let root_folder = path.to_str().ok_or(())?;

        let is_ascii = needle.is_ascii();

        let utf8_algo = move |line: &str, needle: &str, prealloc: &mut (Vec<Score>, Vec<Score>)| {
            if line.len() > max_line_len {
                None
            } else {
                crate::utf8::match_and_score_with_positions(needle, line, prealloc)
            }
        };

        if is_ascii {
            // ascii
            let ascii_algo =
                move |line: &[u8], needle: &[u8], prealloc: &mut (Vec<Score>, Vec<Score>)| {
                    if line.len() > max_line_len {
                        None
                    } else {
                        ascii::match_and_score_with_positions(needle, line, prealloc)
                    }
                };

            let data =
                AsciiSearchData::new(root_folder.into(), needle.into(), ascii_algo, utf8_algo);
            AsciiAlgo::setter(dir_iter, data, r, sort_and_print);
        } else {
            // utf8
            let data = Utf8SearchData::new(root_folder.into(), needle.into(), utf8_algo);
            Utf8Algo::setter(dir_iter, data, r, sort_and_print);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{showcase::*, *};
    use std::time::{Duration, SystemTime};

    #[test]
    fn basic_functionality_test() {
        use std::{cmp::Ordering, io::Write};

        fn insertion_sort_on_sorted(
            global: &mut Vec<MWP>,
            msg: impl IntoIterator<Item = MWP>,
            mut cmp_by: impl FnMut(&MWP, &MWP) -> Ordering,
        ) {
            msg.into_iter().for_each(|x| {
                let idx = global
                    .binary_search_by(|probe| cmp_by(probe, &x))
                    .unwrap_or_else(std::convert::identity);
                global.insert(idx, x);
            });
        };

        fn default_cmp(a: &MWP, b: &MWP) -> Ordering {
            b.1.cmp(&a.1)
        }

        const YOUR_GLOBAL_CAPACITY: usize = 512;
        const YOUR_DYNAMIC_PRINTNUMBER: usize = 8;
        const DELAY: Duration = Duration::from_secs(2);

        macro_rules! test_init {
            ($total: ident, $global_vec: ident, $closure_name:ident; $code:tt) => {{
                let mut $global_vec = Vec::new();
                let mut past = SystemTime::now();
                let mut $total: usize = 0;

                let mut init_flag = true;

                let $closure_name = |msg: Vec<MWP>| {
                    // One-time capacity setter.
                    if init_flag {
                        init_flag = false;
                        $global_vec.reserve(YOUR_GLOBAL_CAPACITY.saturating_sub($global_vec.len()));
                    }
                    // `msglen` will never be bigger than `thread_local_results_cap` from `Rules`,
                    // so `truncate_len` could be evaluated just once:
                    // `global_vec.capacity() - thread_local_results_cap`.
                    let msglen = msg.len();
                    let truncate_len = $global_vec.capacity() - msglen;
                    // If you need to collect all the items without cap,
                    // just reserve `msglen` here instead of truncating.
                    $global_vec.truncate(truncate_len);

                    insertion_sort_on_sorted(&mut $global_vec, msg, default_cmp);
                    $total += msglen;

                    let now = SystemTime::now();

                    if let Ok(dur) = now.duration_since(past) {
                        if dur > DELAY {
                            past = now;

                            let iter = $global_vec.iter().take(YOUR_DYNAMIC_PRINTNUMBER);
                            let stdout = std::io::stdout();
                            let mut stdout = stdout.lock();

                            writeln!(&mut stdout, "Total: {}", $total).unwrap();
                            iter.for_each(|pack| {
                                let (s, _score, pos) = pack;
                                writeln!(&mut stdout, "{}\n{:?}", s, pos).unwrap();
                            });

                            let _ = stdout.flush();
                        }
                    }
                };

                $code
            }};
        }

        let current_dir = std::env::current_dir().unwrap();
        let needle = "print";
        test_init! (
            total, global_vec, sort_and_print;
        {
            default_searcher(current_dir.clone(), needle, sort_and_print).unwrap();
            println!("Total: {}\nCapped results: {:?}", total, global_vec);
        });

        let needle = "sоме Uпiсоdе техт";
        test_init! (
            total, global_vec, sort_and_print;
        {
            with_fzy_algo(current_dir, needle, 1024, sort_and_print).unwrap();
            println!("{:?}", global_vec);
        });
    }
}
