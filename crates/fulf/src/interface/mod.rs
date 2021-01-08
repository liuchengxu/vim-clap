use {
    crate::{
        bytelines::{ByteLines, Line},
        filepath_cache::{IndexedCache, InvalidCache},
        fzy_algo::scoring_utils::{MatchWithPositions, Score, MWP},
    },
    std::{fs, io::Read, mem, path::MAIN_SEPARATOR, sync::Arc, thread},
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
    pub fn new() -> Self {
        Self {
            thread_local_results_cap: 64,
            bonus_threads: if cfg!(target_pointer_width = "64") {
                2
            } else {
                1
            },
        }
    }
}

impl Default for Rules {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone)]
pub struct SpecializedAscii<A, U>
where
    A: Fn(&str, &str, &mut (Vec<Score>, Vec<Score>)) -> Option<MatchWithPositions>
        + Clone
        + Send
        + 'static,
    U: Fn(&str, &str, &mut (Vec<Score>, Vec<Score>)) -> Option<MatchWithPositions>
        + Clone
        + Send
        + 'static,
{
    root_folder: Arc<str>,
    needle: Arc<str>,
    ascii_algo: A,
    fallback_utf8_algo: U,
}

impl<A, U> SpecializedAscii<A, U>
where
    A: Fn(&str, &str, &mut (Vec<Score>, Vec<Score>)) -> Option<MatchWithPositions>
        + Clone
        + Send
        + 'static,
    U: Fn(&str, &str, &mut (Vec<Score>, Vec<Score>)) -> Option<MatchWithPositions>
        + Clone
        + Send
        + 'static,
{
    pub fn new(
        root_folder: Arc<str>,
        needle: Arc<str>,
        ascii_algo: A,
        fallback_utf8_algo: U,
    ) -> Self {
        Self {
            root_folder,
            needle,
            ascii_algo,
            fallback_utf8_algo,
        }
    }

    /// Spawns threads, those threads filter files from the cache.
    pub fn spawner(
        self,
        cache: Arc<IndexedCache>,
        r: Rules,
        handle_results: impl FnMut(Vec<MWP>),
    ) -> Result<(), InvalidCache<()>> {
        let (sx, rx) = flume::bounded((r.bonus_threads as usize + 1) * 2);
        let mut threads = Vec::with_capacity(r.bonus_threads as usize + 1);

        let thread_local_results_cap = r.thread_local_results_cap;

        for _ in 0..r.bonus_threads {
            let t;
            let sender = sx.clone();
            let self_ = self.clone();
            let cache = Arc::clone(&cache);
            t = thread::spawn(move || self_.spawn_me(cache, sender, thread_local_results_cap));

            threads.push(t);
        }
        threads.push(thread::spawn(move || {
            self.spawn_me(cache, sx, thread_local_results_cap)
        }));

        rx.iter().for_each(handle_results);

        let res = threads.into_iter().fold(Ok(()), |res, t| {
            let other = t.join().unwrap();

            if res.is_ok() {
                other
            } else {
                res
            }
        });

        res
    }

    /// Reads the given files and filters them.
    fn spawn_me(
        self,
        files: Arc<IndexedCache>,
        sender: flume::Sender<Vec<MWP>>,
        capnum: usize,
    ) -> Result<(), InvalidCache<()>> {
        let needle: &str = &self.needle;
        let root_folder: &str = &self.root_folder;

        let ascii_algo: A = self.ascii_algo;

        let fallback_utf8_algo: U = self.fallback_utf8_algo;

        let mut prealloc: (Vec<Score>, Vec<Score>) = (Vec::new(), Vec::new());

        let mut inner = Vec::with_capacity(capnum);
        let mut global_linecount: usize = 0;
        let mut filebuf: Vec<u8> = Vec::new();

        let mut files = files.stream_iter()?;
        'file_loop: while let Some(filepath) = files.read_next()? {
            if let Some(_) = fs::File::open(filepath).ok().and_then(|mut file| {
                //x XXX: is megabyte enough for any text file?
                const MEGABYTE: usize = 1_048_576;

                let filesize = initial_buffer_size(&file);
                if filesize > MEGABYTE {
                    return None;
                }

                filebuf.clear();
                filebuf.reserve_exact(filesize);
                file.read_to_end(&mut filebuf).ok()
            }) {
                for (line_idx, line) in ByteLines::new(&filebuf).enumerate() {
                    global_linecount += 1;

                    // There are some mutable borrowing problems,
                    // that this macro solves.
                    macro_rules! apply {
                        ($algo_name:ident, $encoding:expr, $line:expr) => {
                            let algo =
                                |taken_line: &str| $algo_name(taken_line, needle, &mut prealloc);
                            let f = |result| {
                                // Send the results when the buffer is full,
                                // or force-send partial results after some time.
                                if inner.len() == inner.capacity() || global_linecount >= 2048 {
                                    global_linecount = 0;
                                    // Only send non-empty buffers.
                                    if !inner.is_empty() {
                                        let msg =
                                            mem::replace(&mut inner, Vec::with_capacity(capnum));
                                        let _any_result = sender.send(msg);
                                    }
                                }
                                inner.push(result);
                            };

                            apply($encoding, algo, $line, filepath, root_folder, line_idx, f);
                        };
                    }

                    match line {
                        Line::Ascii(line) => {
                            apply!(ascii_algo, Encoding::Ascii, line);
                        }
                        Line::Utf8(line) => {
                            apply!(fallback_utf8_algo, Encoding::Utf8, line);
                        }
                        // Skip the current file if not utf8-encoded.
                        Line::NotUtf8Line => continue 'file_loop,
                    }
                }
            }
        }

        // The last vector could be empty or partially filled.
        if !inner.is_empty() {
            // Whatever is is, we will end this function's work right here anyway.
            let _any_result = sender.send(inner);
        }

        Ok(())
    }
}

// Copypasted from stdlib.
/// Indicates how large a buffer to pre-allocate before reading the entire file.
fn initial_buffer_size(file: &fs::File) -> usize {
    // Allocate one extra byte so the buffer doesn't need to grow before the
    // final `read` call at the end of the file.  Don't worry about `usize`
    // overflow because reading will fail regardless in that case.
    file.metadata().map(|m| m.len() as usize + 1).unwrap_or(0)
}

enum Encoding {
    Ascii,
    Utf8,
}

#[allow(clippy::too_many_arguments)]
fn apply(
    encoding: Encoding,
    mut takes_line: impl FnMut(&str) -> Option<MatchWithPositions>,
    line: &str,
    filepath: &str,
    root_folder: &str,
    line_idx: usize,
    mut f: impl FnMut(MWP),
) {
    if let Some((score, pos)) = takes_line(line) {
        let path_with_root = filepath;

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
        let (trimmed_line, add_col) = match encoding {
            Encoding::Ascii => trim_ascii_whitespace(line),
            Encoding::Utf8 => trim_utf8_whitespace(line),
        };

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

/// Specialized trim function,
/// that counts the number of chars trimmed
/// from the start of the line.
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
        //x on a string that has only whitespaces
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

/// Specialized trim function,
/// that counts the number of chars trimmed
/// from the start of the line.
fn trim_utf8_whitespace(line: &str) -> (&str, usize) {
    let mut trimmed_start: usize = 0;
    let line = line.trim_start_matches(|c: char| {
        let is_w = c.is_whitespace();
        trimmed_start += is_w as usize;

        is_w
    });

    (line.trim_end(), trimmed_start)
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
    let mut index = buf.len();
    let mut u = u;
    while u != 0 {
        index -= 1;
        buf[index] = (u % 10) as u8 + b'0';
        u /= 10;
    }

    // SAFETY: "mod 10 + b'0'" gives only ASCII chars, which is always utf8.
    unsafe { std::str::from_utf8_unchecked_mut(&mut buf[index..]) }
}

/// More of an example, than real thing, yeah. But could be useful.
#[cfg(test)]
mod showcase {
    use super::*;
    use crate::filepath_cache::SerializeError;
    use std::path::Path;

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
    /// `handle_results` - a closure, that takes the results from
    /// busy worker threads and handles those results.
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
        handle_results: impl FnMut(Vec<MWP>),
    ) -> Result<(), SetterError> {
        with_fzy_algo(path, needle, 1024_usize.next_power_of_two(), handle_results)
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

        handle_results: impl FnMut(Vec<MWP>),
    ) -> Result<(), SetterError> {
        use crate::filepath_cache::{serialize, NotUtf8};

        let needle = needle.as_ref();

        if needle.is_empty() || needle.len() > max_line_len {
            return Err(SetterError::WrongSizeNeedle(needle.len()));
        }

        let path = path.as_ref();
        let root_folder = path
            .to_str()
            .ok_or(SetterError::Serialize(SerializeError::NonUtf8Path))?;

        let builder = ignore::WalkBuilder::new(path);
        // Probably, those serialization errors should be handled right there,
        // but for a test it's okay to simply return those errors to the caller.
        let idx_cache = serialize(root_folder, builder, NotUtf8::ReturnError)?;
        let idx_cache = Arc::new(idx_cache);

        // If you don't plan on spawning a new thread to write one
        // little file, passing `Arc` is an overkill.
        let write_cache = |cache: Arc<IndexedCache>| {
            let _bytes_to_write: &[u8] = cache.show_cache();
            /* Angry caching noises. */
            ()
        };
        write_cache(Arc::clone(&idx_cache));

        let utf8_algo = move |line: &str, needle: &str, prealloc: &mut (Vec<Score>, Vec<Score>)| {
            if line.len() > max_line_len {
                None
            } else {
                crate::fzy_algo::utf8::match_and_score_with_positions(needle, line, prealloc)
            }
        };
        let r = Rules::new();

        let is_ascii = needle.is_ascii();
        if is_ascii {
            // ascii
            let ascii_algo =
                move |line: &str, needle: &str, prealloc: &mut (Vec<Score>, Vec<Score>)| {
                    if line.len() > max_line_len {
                        None
                    } else {
                        crate::fzy_algo::ascii::match_and_score_with_positions(
                            needle.as_bytes(),
                            line.as_bytes(),
                            prealloc,
                        )
                    }
                };

            let spec =
                SpecializedAscii::new(root_folder.into(), needle.into(), ascii_algo, utf8_algo);
            spec.spawner(idx_cache, r, handle_results).unwrap();
        } else {
            // utf8
            let unspec = SpecializedAscii::new(
                root_folder.into(),
                needle.into(),
                // Just drop utf8 algorithm in both slots,
                // and that algorithm will run for all lines.
                utf8_algo,
                utf8_algo,
            );
            unspec.spawner(idx_cache, r, handle_results).unwrap();
        }

        Ok(())
    }

    #[derive(Debug)]
    pub enum SetterError {
        WrongSizeNeedle(usize),
        Serialize(SerializeError),
        InvalidCache,
    }
    impl From<InvalidCache<()>> for SetterError {
        fn from(_: InvalidCache<()>) -> Self {
            Self::InvalidCache
        }
    }
    impl From<SerializeError> for SetterError {
        fn from(e: SerializeError) -> Self {
            Self::Serialize(e)
        }
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
            total, global_vec, handle_results;
        {
            default_searcher(current_dir.clone(), needle, handle_results).unwrap();
            println!("Total: {}\nCapped results: {:?}", total, global_vec);
        });

        let needle = "sоме Uпiсоdе техт";
        test_init! (
            total, global_vec, handle_results;
        {
            with_fzy_algo(current_dir, needle, 1024, handle_results).unwrap();
            println!("{:?}", global_vec);
        });
    }
}
