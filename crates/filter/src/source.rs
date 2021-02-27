use std::io::BufRead;
use std::path::PathBuf;

#[cfg(feature = "enable_dyn")]
use subprocess::Exec;

use super::*;

/// Source is anything that can produce an iterator of String.
#[derive(Debug)]
pub enum Source<I: Iterator<Item = SourceItem>> {
    Stdin,
    #[cfg(feature = "enable_dyn")]
    Exec(Box<Exec>),
    File(PathBuf),
    List(I),
}

impl<I: Iterator<Item = SourceItem>> From<PathBuf> for Source<I> {
    fn from(fpath: PathBuf) -> Self {
        Self::File(fpath)
    }
}

#[cfg(feature = "enable_dyn")]
impl<I: Iterator<Item = SourceItem>> From<Exec> for Source<I> {
    fn from(exec: Exec) -> Self {
        Self::Exec(Box::new(exec))
    }
}

impl<I: Iterator<Item = SourceItem>> Source<I> {
    /// Returns the complete filtered results after applying the specified
    /// matcher algo on each item in the input stream.
    ///
    /// This is kind of synchronous filtering, can be used for multi-staged processing.
    pub fn filter(self, matcher: Matcher, query: &str) -> Result<Vec<FilterResult>> {
        let do_match = |line: &str| matcher.do_match(&line.into(), query);

        let filtered = match self {
            Self::Stdin => std::io::stdin()
                .lock()
                .lines()
                .filter_map(|lines_iter| {
                    lines_iter.ok().and_then(|line| {
                        do_match(&line).map(|(score, indices)| (line.into(), score, indices))
                    })
                })
                .collect::<Vec<_>>(),
            #[cfg(feature = "enable_dyn")]
            Self::Exec(exec_cmd) => std::io::BufReader::new(exec_cmd.stream_stdout()?)
                .lines()
                .filter_map(|lines_iter| {
                    lines_iter.ok().and_then(|line| {
                        do_match(&line).map(|(score, indices)| (line.into(), score, indices))
                    })
                })
                .collect::<Vec<_>>(),
            Self::File(fpath) => std::fs::read_to_string(fpath)?
                .par_lines()
                .filter_map(|line| {
                    do_match(&line)
                        .map(|(score, indices)| (line.to_string().into(), score, indices))
                })
                .collect::<Vec<_>>(),
            Self::List(list) => list
                .filter_map(|item| {
                    let line = item.match_text();
                    do_match(line).map(|(score, indices)| (line.into(), score, indices))
                })
                .collect::<Vec<_>>(),
        };

        Ok(filtered)
    }
}
