//! A time-space balanced (de)serialization of the filetree.
//!
//! It's optimized for three things:
//!
//! * fast load with little RAM usage and without reallocations;
//!
//! * effective multithreaded yet unique reads of the files in the tree
//!   along with automatic workload rebalancing (both on per-folder basis);
//!
//! * fast on-load validation (the cache is not guaranteed to be valid after
//!   the index-generating phase, but almost all invalid caches
//!   would not go past that phase).
//!
//! # Byte representation
//!
//! First, let's call the "byte-encoded folder with the files in it" a "chunk".
//!
//! Every chunk is represented as a group of
//!
//! 1. "Byte zero" — a marker, shows the start (and the end) of each chunk.
//!    It's always a byte with value `0` (or `b'\0'`).
//!    That byte exists mostly (but not only) to check
//!    the validness of the cache during the index-generating phase.
//!
//! 2. Length of chunk —
//!    a `usize` number that you should add to the current "byte zero"'s
//!    index to jump to the next byte zero.
//!
//! 3. Folder's length and path — a byte or usize, followed by the utf8
//!    representation of the folder's path without the root path.
//!
//! 4. Arbitrary number of the files' lengths and names —
//!    a sequence of pairs of byte or usize,
//!    followed by the utf8 representation of the file.
//!    Since filenames can't have zero length, the end of the sequence
//!    could be clearly indicated with the "byte zero".
//!
//! 5. Ending byte zero — either the next chunk's starting byte zero,
//!    or the last byte before EOF.
//!    Just like the starting byte zero, that byte exists mostly to check
//!    the validness of the cache during the index-generating phase.
//!
//! Now, that you know what chunks are, there's the layout in chunks:
//!
//! * the first chunk is special: it never contains any files,
//!   only the base folder's path;
//!
//! * all other chunks are all the same.
//!
//! Yes, even the second chunk, that could represent the base folder
//! (doing so with an empty folder's path and non-empty list of files).
//!
//! # Index phase and reads from different threads
//!
//! Reads are made non-blocking yet synchronized, with a simple
//! technique: all indicies of the chunks are collected into the vector,
//! and then those indicies are accessed with `AtomicUsize`.
//!
//! While indicies are collected, the cache validness is checked with
//! positions of zero bytes. This does not guarantee validness, but
//! will catch most invalid caches.
//!
//! # Byte or usize
//!
//! Almost all filenames and even most folder paths would never take more than
//! 127 bytes. Thus, to save a lot of bytes, the length of those names is
//! encoded with a variable-length number.
//!
//! ## Encoding "u8|usize" number
//!
//! If the length fits into the `0..=127` range, then the byte is enough,
//! and the length is just written as a byte.
//!
//! If the length doesn't fit into that range, then the last bit of the
//! string's length is set, and that `usize` is stored via
//! `to_be_bytes()` method, so the most significant byte will be read first.
//!
//! Note: usize minus one bit (`0..=isize::MAX` range) is guaranteed to fit
//! any string's length, if that string is allocated by the default Rust allocator.
//! Anyway, if a user has a path with more than isize::MAX bytes in it,
//! such user has problems much worse than unworking cacher.
//!
//! ## Decoding "u8|usize" number
//!
//! Do all the things as with encoding, but the other way around.
//!
//! If the highest bit of the byte is unset (zero),
//! then the length fits in the `0..=127` range: the byte is ready to use.
//!
//! If the highest bit of the byte is set (one),
//! then the length didn't fit in the `0..=127` range:
//! the byte is taken with the next `size_of::<usize>() - 1` bytes,
//! those bytes are turned into `usize` via `from_be_bytes()`,
//! and we unset the most significant bit in that `usize`. The length is ready.
//!
//! # Path encoding
//!
//! Only paths that could be represented as utf-8 are supported.
//!
//! If a path in the tree cannot be represented as utf-8,
//! it will return an error or just be ignored, based on the option choosen.

use {
    ignore,
    inlinable_string::{InlinableString as InString, StringExt},
    std::{
        cmp::Ordering as CmpOrd,
        mem,
        path::{self, MAIN_SEPARATOR},
        sync::atomic::AtomicUsize,
    },
};

/// `size_of::<usize>()`
const USIZE_SIZE: usize = mem::size_of::<usize>();

/// A `0_u8` in the `ByteOrUsize` representation.
const BYTE_ZERO: ByteOrUsize = ByteOrUsize::ZeroByte;

/// The byte with only last bit set.
const LAST_BIT: u8 = 1_u8.reverse_bits();

/// Defines the behavior of [`serialize`] function,
/// when it meets a path that is not UTF-8 encoded.
///
/// [`serialize`]: fn.serialize.html
pub enum NotUtf8 {
    IgnorePath,
    ReturnError,
}

/// Errors, that could occur during serialization.
#[derive(Debug)]
pub enum SerializeError {
    /// The inner error of the iterator.
    Walk(ignore::Error),
    /// Path that cannot be encoded as UTF-8 was met.
    /// Pretty much self-descriptive.
    NonUtf8Path,
    /// An error, that should never happen, actually.
    /// For more info about the case where this error could happen, read [this].
    ///
    /// [this]: https://docs.rs/ignore/0.4.15/ignore/struct.DirEntry.html#method.file_type
    StdinEntry,
}

impl From<ignore::Error> for SerializeError {
    fn from(e: ignore::Error) -> Self {
        Self::Walk(e)
    }
}

/// If `non_utf8_path` is `ReturnError`,
/// then `ignore::Error::IO(InvalidData)` error will be returned.
///
/// If it is `IgnorePath`,
/// then those paths are ignored, and the serialization continues.
///
/// If the `ignore::Walk` iterator meets any error, that error is returned.
///
/// # Symlinks and added paths
///
/// This algorithm does not support both.
///
/// Thus enabling [link jumps] or [adding] other paths to the builder,
/// will lead to logic bugs, where all files behind the symlink or
/// in the added path would not be read.
///
/// If you need to read files in more than one base folder,
/// create a cache for each such folder
/// and process all needed caches separately, one-by-one.
///
/// [adding]: https://docs.rs/ignore/0.4.15/ignore/struct.WalkBuilder.html#method.add
/// [link jumps]: https://docs.rs/ignore/0.4.15/ignore/struct.WalkBuilder.html#method.follow_links
///
/// # Overrides
///
/// This function overrides any preset sort in the `WalkBuilder`
/// with "files before folders" sort.
pub fn serialize(
    base_folder: impl AsRef<str>,
    mut builder: ignore::WalkBuilder,
    not_utf8_path: NotUtf8,
) -> Result<IndexedCache, SerializeError> {
    macro_rules! not_utf8 {
        () => {
            match not_utf8_path {
                NotUtf8::ReturnError => return Err(SerializeError::NonUtf8Path),
                NotUtf8::IgnorePath => continue,
            }
        };
    }

    let base_folder = append_separator(InString::from(base_folder.as_ref()));

    let mut indicies: Vec<usize> = Vec::new();
    let mut cache: Vec<u8> = Vec::with_capacity(1024);
    write_base_folder(base_folder.as_ref(), &mut cache);

    let mut current_folder = FolderWithfFiles::new(InString::from(""));

    let iter = builder
        .sort_by_file_path(|a_path, b_path| {
            match (a_path.is_file(), b_path.is_file()) {
                (true, true) | (false, false) => a_path.cmp(b_path),
                // Files in the current directory should always go before
                // anything else.
                (true, false) => CmpOrd::Less,
                (false, true) => CmpOrd::Greater,
            }
        })
        .build();

    for dir_ent in iter {
        let dir_ent = dir_ent?;

        match dir_ent.file_type() {
            Some(filetype) => {
                if filetype.is_dir() {
                    current_folder.write_chunk_to(&mut cache, &mut indicies);

                    match dir_ent.path().as_os_str().to_str() {
                        Some(s) => match s.get(base_folder.len()..) {
                            Some(s) => {
                                current_folder =
                                    FolderWithfFiles::new(append_separator(InString::from(s)))
                            }
                            // I told them to not use symlinks and additional paths.
                            // Too bad they didn't read the docs of the function.
                            None => continue,
                        },

                        None => not_utf8!(),
                    };
                } else if filetype.is_file() {
                    match dir_ent.file_name().to_str() {
                        Some(s) => current_folder.push(s),
                        None => not_utf8!(),
                    };
                }
            }
            None => return Err(SerializeError::StdinEntry),
        }
    }

    current_folder.write_chunk_to(&mut cache, &mut indicies);

    Ok(IndexedCache::new(cache, indicies))
}

/// The base folder is unique: it's the only folder that has no files in it;
/// it has a starting byte zero, whether all other folders have only ending byte zero.
fn write_base_folder(base_folder: &str, cache: &mut Vec<u8>) {
    let base_folder_len = ByteOrUsize::new(base_folder.len());

    BYTE_ZERO.write_to(cache);
    cache.extend_from_slice(&to_bytes(
        1 + USIZE_SIZE + base_folder_len.writelen() + base_folder.len(),
    ));
    base_folder_len.write_to(cache);
    cache.extend_from_slice(append_separator(InString::from(base_folder)).as_bytes());
    BYTE_ZERO.write_to(cache);
}

/// If the string doesn't end with a separator,
/// the [`MAIN_SEPARATOR`] is pushed to the end of string.
///
/// [`MAIN_SEPARATOR`]: https://doc.rust-lang.org/std/path/constant.MAIN_SEPARATOR.html
fn append_separator(mut folder_name: InString) -> InString {
    if folder_name.ends_with(path::is_separator) {
        folder_name
    } else {
        folder_name.push(MAIN_SEPARATOR);
        folder_name
    }
}

#[derive(Default)]
struct FolderWithfFiles {
    foldername: InString,
    filenames: Vec<InString>,
}

impl FolderWithfFiles {
    fn new(foldername: InString) -> Self {
        Self {
            foldername,
            filenames: Vec::new(),
        }
    }

    fn push(&mut self, file: impl AsRef<str>) {
        let file = file.as_ref();
        let filename_idx = file
            .char_indices()
            .rfind(|(_idx, c)| *c == MAIN_SEPARATOR)
            .map(|(idx, c)| idx + c.len_utf8())
            .unwrap_or(0);

        self.filenames.push(InString::from(&file[filename_idx..]));
    }

    fn write_chunk_to(&self, v: &mut Vec<u8>, indicies: &mut Vec<usize>) {
        // We are writing a base chunk, not the special first
        // with the rootfolder but without files.
        if self.filenames.is_empty() {
            return;
        }

        const WLENGTH_DEFAULT: [u8; USIZE_SIZE] = [0; USIZE_SIZE];
        // Write zeroes as the chunk's length, and remember the position.
        // Those zeroes will be rewritten with the real length later.
        let chunk_len_range = v.len()..(v.len() + USIZE_SIZE);
        v.extend_from_slice(&WLENGTH_DEFAULT);
        // Write the filelength index of the current chunk.
        indicies.push(v.len());

        let mut chunk_len = 1 + USIZE_SIZE;

        // Write the length and contents of the folder's name.
        let foldername_len = ByteOrUsize::new(self.foldername.len());
        chunk_len += foldername_len.writelen() + self.foldername.len();
        foldername_len.write_to(v);
        v.extend_from_slice(self.foldername.as_bytes());

        // Write the length and contents of each filename.
        self.filenames.iter().for_each(|name| {
            let namelen = ByteOrUsize::new(name.len());
            chunk_len += namelen.writelen() + name.len();
            namelen.write_to(v);
            v.extend_from_slice(name.as_bytes());
        });

        // Write the chunk's length.
        v[chunk_len_range].copy_from_slice(&to_bytes(chunk_len));
        // Write the zero byte.
        BYTE_ZERO.write_to(v);
    }
}

/// Parses the lone vector of bytes into the cache.
///
/// Returns error with the vector inside, if the cache is invalid.
pub fn deserialize(bytes: Vec<u8>) -> Result<IndexedCache, InvalidCache<Vec<u8>>> {
    macro_rules! invalid {
        () => {
            return Err(InvalidCache(bytes));
        };
    }

    let mut cacheslice: &[u8] = &bytes;

    let mut indicies: Vec<usize> = Vec::with_capacity(8);
    let mut index_of_first_byte_of_folder_length = 1 + USIZE_SIZE;

    loop {
        match cacheslice {
            [0] => break,
            [0, xs @ ..] => match read_usize(xs) {
                // zero byte (1) + usize itself (USIZE_SIZE)
                // + at least one byte indicating folder's length
                Ok(chunk_len) if chunk_len < 1 + USIZE_SIZE + 1 => invalid!(),
                Ok(u) => match cacheslice.get(u..) {
                    Some(slice) => {
                        cacheslice = slice;
                        indicies.push(index_of_first_byte_of_folder_length);
                        index_of_first_byte_of_folder_length += u;
                        continue;
                    }
                    None => invalid!(),
                },
                Err(_) => invalid!(),
            },
            _ => invalid!(),
        }
    }

    Ok(IndexedCache::new(bytes, indicies))
}

/// The error, indicating the invalidness of the cache.
#[derive(Debug)]
pub struct InvalidCache<T>(pub T);

/// The cache, that made it past the index phase.
/// Praise the survivor!
pub struct IndexedCache {
    jumper: AtomicUsize,
    indicies: Vec<usize>,
    cache: Vec<u8>,
}

impl IndexedCache {
    fn new(cache: Vec<u8>, indicies: Vec<usize>) -> Self {
        Self {
            jumper: AtomicUsize::new(0),
            indicies,
            cache,
        }
    }

    /// Shows the whole cache.
    ///
    /// Use it to write the cache into the file.
    pub fn show_cache(&self) -> &[u8] {
        &self.cache
    }

    /// Like `.iter()`, but fallible and produces streaming iterator
    /// instead of simple iterator.
    pub fn stream_iter(&self) -> Result<StreamIter<'_>, InvalidCache<()>> {
        StreamIter::new(self)
    }
}

/// An "easy to do bytetricks and writes" enum.
#[derive(Debug, Clone, Copy)]
enum ByteOrUsize {
    /// A byte. It is always zero.
    ///
    /// It has it's own variant just to make matching against it easier.
    ZeroByte,
    /// Just a byte. It is never zero.
    Byte(u8),
    /// Stored value is already transmuted by `to_bytes()`,
    /// and has the most significant bit set.
    Usize([u8; USIZE_SIZE]),
}
use ByteOrUsize::*;

impl PartialEq<u8> for ByteOrUsize {
    fn eq(&self, other: &u8) -> bool {
        match (self, other) {
            (ZeroByte, 0) => true,
            (Byte(a), b) => a == b,
            _ => false,
        }
    }
}

impl ByteOrUsize {
    /// Returns the usize formatted for the chunk.
    fn new(x: usize) -> Self {
        if x == 0 {
            ZeroByte
        } else if x <= 127 {
            Byte(x as u8)
        } else if x > (isize::MAX as usize) {
            isize_overflow()
        } else {
            Usize({
                let mut bytes = to_bytes(x);
                set_last_bit_of_first_byte(&mut bytes);
                bytes
            })
        }
    }

    #[allow(clippy::trivially_copy_pass_by_ref)]
    fn writelen(&self) -> usize {
        match self {
            ZeroByte | Byte(_) => 1,
            Usize(_) => USIZE_SIZE,
        }
    }

    fn write_to(self, w: &mut Vec<u8>) {
        match self {
            ZeroByte => w.push(0),
            Byte(x) => w.push(x),
            Usize(x) => w.extend_from_slice(&x),
        }
    }

    #[inline]
    fn decode(cache: &[u8]) -> Result<(ByteOrUsize, &[u8]), InvalidCache<()>> {
        match cache.get(0) {
            Some(&byte) => {
                if byte & LAST_BIT == 0 {
                    Ok((if byte == 0 { ZeroByte } else { Byte(byte) }, &cache[1..]))
                } else if cache.len() > USIZE_SIZE {
                    let mut bytes: [u8; USIZE_SIZE] = [0; USIZE_SIZE];
                    bytes.copy_from_slice(&cache[..USIZE_SIZE]);

                    Ok((Usize(bytes), &cache[USIZE_SIZE..]))
                } else {
                    Err(InvalidCache(()))
                }
            }

            None => Err(InvalidCache(())),
        }
    }

    fn as_usize(self) -> usize {
        match self {
            ZeroByte => 0,
            Byte(b) => b as usize,
            Usize(mut bytes) => {
                unset_last_bit_of_first_byte(&mut bytes);
                from_bytes(bytes)
            }
        }
    }
}

#[inline]
fn set_last_bit_of_first_byte(bytes: &mut [u8; USIZE_SIZE]) {
    bytes[0] |= LAST_BIT;
}

#[inline]
fn unset_last_bit_of_first_byte(bytes: &mut [u8; USIZE_SIZE]) {
    bytes[0] &= !LAST_BIT
}

#[inline]
fn to_bytes(x: usize) -> [u8; USIZE_SIZE] {
    x.to_be_bytes()
}

#[inline]
fn from_bytes(bytes: [u8; USIZE_SIZE]) -> usize {
    usize::from_be_bytes(bytes)
}

/// Simply tries to read `usize` value from the given slice.
#[inline]
fn read_usize(cache: &[u8]) -> Result<usize, InvalidCache<()>> {
    cache
        .get(..USIZE_SIZE)
        .map(|s| {
            let mut bytes: [u8; USIZE_SIZE] = [0; USIZE_SIZE];
            bytes.copy_from_slice(s);

            from_bytes(bytes)
        })
        .ok_or(InvalidCache(()))
}

fn isize_overflow() -> ! {
    panic!(
        "cacher does not support paths of length bigger than {}",
        isize::MAX
    )
}

pub use iter::StreamIter;
mod iter {
    use super::{
        ByteOrUsize::{self, *},
        IndexedCache, InvalidCache, USIZE_SIZE,
    };
    use std::{
        str,
        sync::atomic::{
            AtomicUsize,
            Ordering::{AcqRel, Acquire},
        },
    };

    /// An iterator that doesn't implement `Iterator` trait.
    ///
    /// How to use:
    ///
    /// ```ignore
    /// while let Ok(Some(x)) = stream_iter.read_next() {
    ///     /* do thing with x */
    /// }
    /// ```
    #[derive(Debug)]
    pub struct StreamIter<'a> {
        cachebuf: &'a [u8],
        indicies: &'a [usize],
        jumper: &'a AtomicUsize,
        base_folder_len: usize,
        // base folder length + folder length
        trim_filename_len: usize,
        // Points either to the first byte of filename length,
        // or to the byte zero.
        next_filename_index: usize,
        buf: String,
    }

    impl<'a> StreamIter<'a> {
        pub(super) fn new(cache: &'a IndexedCache) -> Result<Self, InvalidCache<()>> {
            macro_rules! invalid {
                () => {
                    return Err(InvalidCache(()));
                };
            }

            let cachebuf: &[u8] = &cache.cache;
            let indicies: &[usize] = &cache.indicies;
            let jumper: &AtomicUsize = &cache.jumper;

            match cachebuf.get(..1 + USIZE_SIZE) {
                Some([0, ..]) => {
                    let (base_folder_len, cachebuf_past_len) =
                        ByteOrUsize::decode(&cachebuf[1 + USIZE_SIZE..])?;
                    let base_folder_len = base_folder_len.as_usize();

                    let (base, cache_past_base) = match cachebuf_past_len
                        .get(..base_folder_len)
                        .and_then(|s| str::from_utf8(s).ok())
                    {
                        Some(s) => (s, &cachebuf_past_len[base_folder_len..]),
                        None => invalid!(),
                    };

                    let self_ = Self {
                        cachebuf,
                        indicies,
                        jumper,
                        base_folder_len,
                        next_filename_index: if jumper.load(Acquire) >= indicies.len() {
                            cachebuf.len() - 1
                        } else {
                            cachebuf.len() - cache_past_base.len()
                        },
                        buf: String::from(base),
                        trim_filename_len: base_folder_len,
                    };
                    Ok(self_)
                }
                _ => invalid!(),
            }
        }
    }

    impl StreamIter<'_> {
        /// Like `next`, but can't produce two items at once,
        /// because returned `&str` is borrowed from the iterator itself.
        pub fn read_next(&mut self) -> Result<Option<&str>, InvalidCache<()>> {
            macro_rules! invalid {
                () => {
                    return Err(InvalidCache(()));
                };
            }

            match self.cachebuf.get(self.next_filename_index..) {
                Some(slice) => match ByteOrUsize::decode(slice)? {
                    // The final zero byte.
                    (ZeroByte, []) => Ok(None),
                    // Get the folder with next index.
                    // Push that folder into string.
                    // Then go with the filenames again.
                    (ZeroByte, _) => {
                        let next_jump = self.jumper.fetch_add(1, AcqRel);
                        if next_jump >= self.indicies.len() {
                            self.jumper.fetch_sub(1, AcqRel);
                            self.next_filename_index = self.cachebuf.len() - 1;
                            return Ok(None);
                        }

                        let new_idx = self.indicies[next_jump];
                        let slice: &[u8] = &self.cachebuf[new_idx..];
                        let (len, folder_slice) = ByteOrUsize::decode(slice)?;
                        let wrlen = len.writelen();
                        let foldername = match folder_slice
                            .get(..len.as_usize())
                            .and_then(|name| str::from_utf8(name).ok())
                        {
                            Some(s) => s,
                            None => invalid!(),
                        };

                        self.next_filename_index = new_idx + wrlen + foldername.len();
                        self.buf.truncate(self.base_folder_len);
                        self.buf.push_str(foldername);
                        self.trim_filename_len = self.buf.len();

                        self.read_next()
                    }

                    // Get the filename, push the filename into string,
                    // move the filename index, return the path. Easy!
                    (len, slice) => {
                        let wrlen = len.writelen();
                        let filename = match slice
                            .get(..len.as_usize())
                            .and_then(|filename| str::from_utf8(filename).ok())
                        {
                            Some(s) => s,
                            None => invalid!(),
                        };

                        self.next_filename_index += wrlen + filename.len();
                        self.buf.truncate(self.trim_filename_len);
                        self.buf.push_str(filename);

                        Ok(Some(&self.buf))
                    }
                },
                None => invalid!(),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serialize_deserialize() {
        fn qwe(mut iter: StreamIter) -> Result<(), InvalidCache<()>> {
            while let Some(pathstring) = iter.read_next()? {
                // println!("{}", pathstring);
                let path: &std::path::Path = pathstring.as_ref();
                assert!(path.exists());
            }
            Ok(())
        }

        let mut current_dir = std::env::current_dir().unwrap();
        current_dir.pop();

        let cache = serialize(
            current_dir.as_os_str().to_str().unwrap(),
            ignore::WalkBuilder::new(&current_dir),
            NotUtf8::ReturnError,
        )
        .unwrap();

        let q = qwe(cache.stream_iter().unwrap());
        assert!(q.is_ok());

        println!("\n\n");

        let vec_cache: Vec<u8> = cache.show_cache().to_owned();
        let cache = deserialize(vec_cache).unwrap();

        let q = qwe(cache.stream_iter().unwrap());
        assert!(q.is_ok());
    }

    #[test]
    fn test_multithread_access() {
        fn asd(mut iter: StreamIter) -> Result<Vec<Box<str>>, InvalidCache<()>> {
            let mut v = Vec::new();

            while let Some(pathstring) = iter.read_next()? {
                let path: &std::path::Path = pathstring.as_ref();
                assert!(path.exists());
                v.push(Box::from(pathstring));
            }
            Ok(v)
        }

        use std::{sync::Arc, thread};

        let mut current_dir = std::env::current_dir().unwrap();
        current_dir.pop();

        let cache = serialize(
            current_dir.as_os_str().to_str().unwrap(),
            ignore::WalkBuilder::new(&current_dir),
            NotUtf8::ReturnError,
        )
        .unwrap();

        let cache = Arc::new(cache);

        let handles = (0..15)
            .map(|_| {
                let cache = Arc::clone(&cache);
                thread::spawn(move || {
                    let q = asd(cache.stream_iter().unwrap());
                    assert!(q.is_ok());
                    q.unwrap()
                })
            })
            .collect::<Vec<_>>();

        let mut collected = handles.into_iter().fold(Vec::new(), |mut acc, other_vec| {
            acc.append(&mut other_vec.join().unwrap());
            acc
        });
        assert!(!collected.is_empty());
        collected.sort_unstable();
        collected.windows(2).for_each(|sl| assert_ne!(sl[0], sl[1]));
    }
}
