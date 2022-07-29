use std::io::BufRead;
use std::path::{Path, PathBuf};

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

impl<I: Iterator<Item = SourceItem>> Source<I> {
    /// Returns the complete filtered results given `matcher` and `query`.
    ///
    /// This is kind of synchronous filtering, can be used for multi-staged processing.
    pub fn run_and_collect(self, matcher: Matcher, query: &Query) -> Result<Vec<MatchedItem>> {
        let res = match self {
            Self::List(list) => source_list(&matcher, query, list).collect(),
            Self::Stdin => source_stdin(&matcher, query).collect(),
            Self::File(fpath) => source_file(&matcher, query, fpath)?.collect(),
            Self::Exec(exec) => source_exec(&matcher, query, exec)?.collect(),
        };

        Ok(res)
    }
}

/// Generate an iterator of [`MatchedItem`] from [`Source::List(list)`].
pub fn source_list<'a, 'b: 'a>(
    matcher: &'a Matcher,
    query: &'a Query,
    list: impl Iterator<Item = SourceItem> + 'b,
) -> impl Iterator<Item = MatchedItem> + 'a {
    list.filter_map(|item| matcher.match_item(item, query))
}

/// Generate an iterator of [`MatchedItem`] from [`Source::Stdin`].
pub fn source_stdin<'a>(
    matcher: &'a Matcher,
    query: &'a Query,
) -> impl Iterator<Item = MatchedItem> + 'a {
    std::io::stdin()
        .lock()
        .lines()
        .filter_map(move |lines_iter| {
            lines_iter.ok().and_then(|line: String| {
                let item = SourceItem::from(line);
                matcher.match_item(item, query)
            })
        })
}

/// Generate an iterator of [`MatchedItem`] from [`Source::File`].
pub fn source_file<'a, P: AsRef<Path>>(
    matcher: &'a Matcher,
    query: &'a Query,
    path: P,
) -> Result<impl Iterator<Item = MatchedItem> + 'a> {
    // To avoid Err(Custom { kind: InvalidData, error: "stream did not contain valid UTF-8" })
    // The line stream can contain invalid UTF-8 data.
    Ok(std::io::BufReader::new(std::fs::File::open(path)?)
        .lines()
        .filter_map(|x| {
            x.ok().and_then(|line: String| {
                let item = SourceItem::from(line);
                matcher.match_item(item, query)
            })
        }))
}

/// Generate an iterator of [`MatchedItem`] from [`Source::Exec`].
pub fn source_exec<'a>(
    matcher: &'a Matcher,
    query: &'a Query,
    exec: Box<Exec>,
) -> Result<impl Iterator<Item = MatchedItem> + 'a> {
    Ok(std::io::BufReader::new(exec.stream_stdout()?)
        .lines()
        .filter_map(|lines_iter| {
            lines_iter.ok().and_then(|line: String| {
                let item = SourceItem::from(line);
                matcher.match_item(item, query)
            })
        }))
}
