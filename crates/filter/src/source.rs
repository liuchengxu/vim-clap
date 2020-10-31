use super::*;
use crate::SourceItem;
use matcher::{fzy, skim, substring};
use std::io::BufRead;
use std::path::PathBuf;
#[cfg(feature = "enable_dyn")]
use subprocess::Exec;

/// Source is anything that can produce an iterator of String.
#[derive(Debug)]
pub enum Source<I: Iterator<Item = SourceItem>> {
    Stdin,
    #[cfg(feature = "enable_dyn")]
    Exec(Exec),
    File(PathBuf),
    List(I),
}

impl From<Vec<String>> for Source<std::vec::IntoIter<SourceItem>> {
    fn from(source_list: Vec<String>) -> Self {
        // XXX: Not sure about the .into_iter().map().collect().into_iter(), pretty sure it's doing
        // something wasteful.
        Self::List(source_list
            .into_iter()
            .map(|line| SourceItem { display: line, filter: None })
            .collect::<Vec<_>>()
            .into_iter())
    }
}

impl<I: Iterator<Item = SourceItem>> From<PathBuf> for Source<I> {
    fn from(fpath: PathBuf) -> Self {
        Self::File(fpath)
    }
}

#[cfg(feature = "enable_dyn")]
impl<I: Iterator<Item = SourceItem>> From<Exec> for Source<I> {
    fn from(exec: Exec) -> Self {
        Self::Exec(exec)
    }
}

impl<I: Iterator<Item = SourceItem>> Source<I> {
    /// Returns the complete filtered results after applying the specified
    /// matcher algo on each item in the input stream.
    ///
    /// This is kind of synchronous filtering, can be used for multi-staged processing.
    pub fn filter(self, algo: Algo, query: &str) -> Result<Vec<FilterResult>> {
        let matcher = |line: &str| match algo {
            Algo::Skim => skim::fuzzy_indices(line, &query),
            Algo::Fzy => fzy::fuzzy_indices(line, &query),
            Algo::SubString => substring::substr_indices(line, &query),
        };

        let filtered = match self {
            Self::Stdin => std::io::stdin()
                .lock()
                .lines()
                .filter_map(|lines_iter| {
                    lines_iter.ok().and_then(|line| {
                        matcher(&line).map(|(score, indices)| (SourceItem::new(line), score, indices))
                    })
                })
                .collect::<Vec<_>>(),
            #[cfg(feature = "enable_dyn")]
            Self::Exec(exec_cmd) => std::io::BufReader::new(exec_cmd.stream_stdout()?)
                .lines()
                .filter_map(|lines_iter| {
                    lines_iter.ok().and_then(|line| {
                        matcher(&line).map(|(score, indices)| (SourceItem::new(line), score, indices))
                    })
                })
                .collect::<Vec<_>>(),
            Self::File(fpath) => std::fs::read_to_string(fpath)?
                .par_lines()
                .filter_map(|line| {
                    matcher(&line).map(|(score, indices)| (SourceItem::new(line.into()), score, indices))
                })
                .collect::<Vec<_>>(),
            Self::List(list) => list
                .filter_map(|item| {
                    // XXX: How do I avoid cloning here?
                    let line = item.filter.clone().unwrap_or(item.display.clone());
                    matcher(&line).map(|(score, indices)| (item, score, indices))
                })
                .collect::<Vec<_>>(),
        };

        Ok(filtered)
    }
}
