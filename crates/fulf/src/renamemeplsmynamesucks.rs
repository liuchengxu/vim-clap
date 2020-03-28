use {
    crate::{
        ascii::{self, bytes_into_ascii_string_lossy},
        fileworks::ByteLines,
        scoring_utils::Score,
        threadworks::{MWPuple, MWPutex, ThreadMe, Threader},
        utf8::{self, NeedleUTF8},
    },
    ignore,
    std::{
        ops::DerefMut,
        sync::{Arc, Mutex, MutexGuard},
    },
};

type ScoringResult = (Box<str>, Score, Box<[usize]>);
type MWP = ScoringResult;

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
    /// 2. This a non-formatted line of automatically generated code.
    ///
    /// 3. This is a very bad code.
    ///
    /// 4. Some very rare other reasons.
    ///
    /// And in any of those cases there's probably no point in fuzzing such line.
    pub max_line_len: usize,

    /// If set to `false`, any error will be ignored, if it didn't cause panic
    /// in the main thread.
    ///
    /// If set to `true`, all errors are stored and returned,
    /// so you can do whatever you want with them.
    ///
    /// Ignoring errors is probably faster.
    pub with_errors: bool,

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
            && self.with_errors == other.with_errors
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
            results_cap: 0,
            max_line_len: 0,
            with_errors: false,
            bonus_threads: 0,
        }
    }
}

struct NoError;

impl From<std::io::Error> for NoError {
    #[inline]
    fn from(_: std::io::Error) -> Self {
        Self
    }
}

//XXX Attention, please!
//XXX Those two macros are quite similar, thus to not do something wrong occasionally,
//XXX those macros have different order of elements. Please, if you change the macro,
//XXX do not make number and order of elements the same.
macro_rules! ascii {
    ($dir_walker:expr, $needle:expr, $capnum:expr, $shared:expr, $threadcount:expr, $max_line_len:expr, $printer:expr) => {{
        let (dir_walker, needle, capnum, shared, threadcount, max_line_len, printer) = (
            $dir_walker,
            $needle,
            $capnum,
            $shared,
            $threadcount,
            $max_line_len,
            $printer,
        );

        let needle = Box::<[u8]>::from(needle);

        let closure = move |v: Vec<u8>, inner: &mut Vec<MWP>, shared: &Mutex<_>| {
            ByteLines::new(v.as_slice()).for_each(|line| {
                if line.len() > max_line_len {
                    return;
                }

                if let Some(()) = ascii::matcher(line, needle.as_ref()) {
                    let (score, posits) = ascii::score_with_positions(needle.as_ref(), line);
                    inner.push((
                        bytes_into_ascii_string_lossy(line.to_owned()).into_boxed_str(),
                        score,
                        posits.into_boxed_slice(),
                    ));

                    let inner_len = inner.len();
                    if inner_len == inner.capacity() {
                        append_to_shared(shared, inner, printer);
                    }
                }
            });
        };

        let threadme = ThreadMe::new(
            capnum,
            Mutex::new(dir_walker),
            shared,
            closure,
            append_to_shared,
            printer,
        );
        let threadme = Arc::new(threadme);

        let errors = Threader::run_chain(Arc::clone(&threadme), threadcount);

        // This should never return error.
        let results = match Arc::try_unwrap(threadme) {
            Ok(o) => o.into_shared(),
            _ => panic!(),
        };

        (results, errors)
    }};
}

//XXX Attention, please!
//XXX Those two macros are quite similar, thus to not do something wrong occasionally,
//XXX those macros have different order of elements. Please, if you change the macro,
//XXX do not make number and order of elements the same.
macro_rules! utf8 {
    ($needle:expr, $capnum:expr, $dir_walker:expr, $threadcount:expr, $shared:expr, $printer:expr, $max_line_len:expr) => {{
        let (needle, capnum, dir_walker, shared, threadcount, printer, max_line_len) = (
            $needle,
            $capnum,
            $dir_walker,
            $shared,
            $threadcount,
            $printer,
            $max_line_len,
        );

        let (needle, needle_len) = match NeedleUTF8::new(needle) {
            Some(n) => n,
            // Empty needle if none.
            None => return ((Vec::new(), 0), Vec::new()),
        };

        let closure = move |v: Vec<u8>, inner: &mut Vec<MWP>, shared: &Mutex<_>| {
            ByteLines::new(v.as_slice()).for_each(|line| {
                if line.len() > max_line_len {
                    return;
                }

                if let Some(()) = utf8::matcher(line, needle.as_matcher_needle()) {
                    let (score, posits) = utf8::score_with_positions(
                        needle.as_ref(),
                        needle_len,
                        String::from_utf8_lossy(line).as_ref(),
                    );
                    inner.push((
                        String::from_utf8_lossy(line).into_owned().into_boxed_str(),
                        score,
                        posits.into_boxed_slice(),
                    ));

                    let inner_len = inner.len();
                    if inner_len == inner.capacity() {
                        append_to_shared(shared, inner, printer);
                    }
                }
            });
        };

        let threadme = ThreadMe::new(
            capnum,
            Mutex::new(dir_walker),
            shared,
            closure,
            append_to_shared,
            printer,
        );
        let threadme = Arc::new(threadme);

        let errors = Threader::run_chain(Arc::clone(&threadme), threadcount);

        // This should never return error.
        let results = match Arc::try_unwrap(threadme) {
            Ok(o) => o.into_shared(),
            _ => panic!(),
        };

        (results, errors)
    }};
}

/// Probably, the main function there.
///
/// Directory iterator and `Rules` are simple, but `printer()` is tricky:
/// it takes a slice with already sorted results (bigger score at the start),
/// and a number of total lines that provided any score.
///
/// `printer()` has very strict bounds;
/// probably only safe static function could satisfy those.
#[inline]
pub fn starter<F>(
    dir_walker: ignore::Walk,
    r: Rules,
    printer: F,
) -> (MWPuple<MWP>, Vec<ignore::Error>)
where
    F: Fn(&[MWP], usize) + Send + Sync + 'static + Copy,
{
    let capnum = r.results_cap;
    let shared: MWPutex<MWP> = Mutex::new((Vec::with_capacity(capnum * 2), 0));
    let needle = r.needle;
    let threadcount = r.bonus_threads;
    let max_line_len = r.max_line_len;

    if needle.is_empty() {
        return Default::default();
    }

    // `ThreadMe` needs two functions or closures:
    //
    // F: Fn(Vec<u8>, &mut Vec<S>) + Send + Sync + 'static, // do_text_into_inner: F,
    // G: Fn(&Mutex<Vec<S>>, &mut Vec<S>) + Send + Sync + 'static, // do_shared_and_inner: G,
    //
    // First one takes a whole file's buffer with mutable access to inner thread's score-storage.
    //
    // Second takes a reference to shared buffer behind a mutex,
    // and a mutable access to the inner thread's score-storage.
    //

    match r.with_errors {
        // Make errors empty and return empty vector.
        false => {
            let dir_walker = dir_walker.map(|res| {
                if let Ok(res) = res {
                    Ok(res.into_path())
                } else {
                    Err(NoError)
                }
            });

            // Errors are ignored, so this match won't return errors you can work with.
            let (results, _errors) = match (r.force_utf8, needle.is_ascii()) {
                // ascii
                (false, true) => ascii!(
                    dir_walker,
                    needle,
                    capnum,
                    shared,
                    threadcount,
                    max_line_len,
                    printer
                ),
                // utf8
                (true, _) | (_, false) => utf8!(
                    needle,
                    capnum,
                    dir_walker,
                    threadcount,
                    shared,
                    printer,
                    max_line_len
                ),
            };

            (results, Vec::new())
        }

        // Return errors.
        true => {
            let dir_walker = dir_walker.map(|res| match res {
                Ok(res) => Ok(res.into_path()),
                Err(e) => Err(e),
            });

            let (results, errors) = match (r.force_utf8, needle.is_ascii()) {
                // ascii
                (false, true) => ascii!(
                    dir_walker,
                    needle,
                    capnum,
                    shared,
                    threadcount,
                    max_line_len,
                    printer
                ),
                // utf8
                (true, _) | (_, false) => utf8!(
                    needle,
                    capnum,
                    dir_walker,
                    threadcount,
                    shared,
                    printer,
                    max_line_len
                ),
            };

            (results, errors)
        }
    }
}

#[inline]
// let append_to_shared = |shared: &Mutex<Vec<MWP>>, unshared: &mut Vec<MWP>| {
fn append_to_shared<F>(shared: &MWPutex<MWP>, unshared: &mut Vec<MWP>, printer: F)
where
    F: Fn(&[MWP], usize),
{
    if !unshared.is_empty() {
        let mut lock = lock_any(shared);
        let (sh, total) = lock.deref_mut();
        let inner_len = unshared.len();

        *total = total.wrapping_add(inner_len);

        sh.append(unshared);
        sh.sort_unstable_by(|a, b| b.1.cmp(&a.1));
        let len = sh.capacity() / 2;
        sh.truncate(len);

        printer(sh, total.clone());

        drop(lock);
    }
}

#[inline]
fn lock_any<T>(m: &Mutex<T>) -> MutexGuard<T> {
    match m.lock() {
        Ok(g) => g,
        Err(pois) => pois.into_inner(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_functionality_test() {
        fn pointer(a: &[MWP], b: usize) {
            println!("Total: {};\nTop results:\n{:?}", b, a);
        }

        let a = starter(
            ignore::Walk::new(std::env::current_dir().unwrap()),
            Rules::default(),
            pointer,
        );

        println!("{:?}", a);
    }
}
