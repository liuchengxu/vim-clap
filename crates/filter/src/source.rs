use std::io::BufRead;
use std::path::PathBuf;

use anyhow::Result;
use subprocess::Exec;

use matcher::Matcher;
use types::{MatchedItem, Query, SourceItem};

/// Source is anything that can produce an iterator of String.
#[derive(Debug)]
pub enum Source<I: Iterator<Item = SourceItem>> {
    Stdin,
    Exec(Box<Exec>),
    File(PathBuf),
    List(I),
}

impl<I: Iterator<Item = SourceItem>> From<PathBuf> for Source<I> {
    fn from(fpath: PathBuf) -> Self {
        Self::File(fpath)
    }
}

impl<I: Iterator<Item = SourceItem>> From<Exec> for Source<I> {
    fn from(exec: Exec) -> Self {
        Self::Exec(Box::new(exec))
    }
}

/// macros for `dyn_collect_number` and `dyn_collect_number`
///
/// Generate an iterator of [`MatchedItem`] from [`Source::Stdin`].
#[macro_export]
macro_rules! source_iter_stdin {
    ( $scorer:ident ) => {
        std::io::stdin().lock().lines().filter_map(|lines_iter| {
            lines_iter
                .ok()
                .map(Into::<SourceItem>::into)
                .and_then(|item| {
                    $scorer(&item).map(|match_result| match_result.into_filtered_item(item))
                })
                .map(Into::into)
        })
    };
}

/// Generate an iterator of [`MatchedItem`] from [`Source::Exec`].
#[macro_export]
macro_rules! source_iter_exec {
    ( $scorer:ident, $exec:ident ) => {
        std::io::BufReader::new($exec.stream_stdout()?)
            .lines()
            .filter_map(|lines_iter| {
                lines_iter
                    .ok()
                    .map(Into::<SourceItem>::into)
                    .and_then(|item| {
                        $scorer(&item).map(|match_result| match_result.into_filtered_item(item))
                    })
                    .map(Into::into)
            })
    };
}

/// Generate an iterator of [`MatchedItem`] from [`Source::File`].
#[macro_export]
macro_rules! source_iter_file {
    ( $scorer:ident, $fpath:ident ) => {
        // To avoid Err(Custom { kind: InvalidData, error: "stream did not contain valid UTF-8" })
        // The line stream can contain invalid UTF-8 data.
        std::io::BufReader::new(std::fs::File::open($fpath)?)
            .lines()
            .filter_map(|x| {
                x.ok()
                    .map(Into::<SourceItem>::into)
                    .and_then(|item| {
                        $scorer(&item).map(|match_result| match_result.into_filtered_item(item))
                    })
                    .map(Into::into)
            })
    };
}

/// Generate an iterator of [`MatchedItem`] from [`Source::List(list)`].
#[macro_export]
macro_rules! source_iter_list {
    ( $scorer:ident, $list:ident ) => {
        $list
            .filter_map(|item| {
                $scorer(&item).map(|match_result| match_result.into_filtered_item(item))
            })
            .map(Into::into)
    };
}

impl<I: Iterator<Item = SourceItem>> Source<I> {
    /// Returns the complete filtered results given `matcher` and `query`.
    ///
    /// This is kind of synchronous filtering, can be used for multi-staged processing.
    pub fn run_and_collect(self, matcher: Matcher, query: &Query) -> Result<Vec<MatchedItem>> {
        let scorer = |item: &SourceItem| matcher.match_item(item, query);

        let filtered = match self {
            Self::Stdin => source_iter_stdin!(scorer).collect(),
            Self::Exec(exec) => source_iter_exec!(scorer, exec).collect(),
            Self::File(fpath) => source_iter_file!(scorer, fpath).collect(),
            Self::List(list) => source_iter_list!(scorer, list).collect(),
        };

        Ok(filtered)
    }
}
