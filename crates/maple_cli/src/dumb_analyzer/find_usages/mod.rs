mod search_engine;

use std::ops::{Index, IndexMut};

use rayon::prelude::*;

pub use self::search_engine::{CtagsSearcher, GtagsSearcher, RegexSearcher, SearchType};

#[derive(Clone, Debug, Default)]
pub struct Usage {
    pub line: String,
    pub indices: Vec<usize>,
}

impl Usage {
    pub fn new(line: String, indices: Vec<usize>) -> Self {
        Self { line, indices }
    }
}

impl PartialEq for Usage {
    fn eq(&self, other: &Self) -> bool {
        // Equal if the path and lnum are the same.
        // [tags]crates/readtags/sys/libreadtags/Makefile:388:1:srcdir
        match (self.line.split_once(']'), other.line.split_once(']')) {
            (Some((_, l1)), Some((_, l2))) => match (l1.split_once(':'), l2.split_once(':')) {
                (Some((l_path, l_1)), Some((r_path, r_1))) if l_path == r_path => {
                    matches!(
                      (l_1.split_once(':'), r_1.split_once(':')),
                      (Some((l_lnum, _)), Some((r_lnum, _))) if l_lnum == r_lnum
                    )
                }
                _ => false,
            },
            _ => false,
        }
    }
}

impl Eq for Usage {}

/// All the lines as well as their match indices that can be sent to the vim side directly.
#[derive(Clone, Debug, Default)]
pub struct Usages(Vec<Usage>);

impl From<Vec<Usage>> for Usages {
    fn from(inner: Vec<Usage>) -> Self {
        Self(inner)
    }
}

impl Index<usize> for Usages {
    type Output = Usage;
    fn index(&self, index: usize) -> &Self::Output {
        &self.0[index]
    }
}

impl IndexMut<usize> for Usages {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.0[index]
    }
}

impl Usages {
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn contains(&self, ele: &Usage) -> bool {
        self.0.contains(ele)
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn iter(&self) -> impl Iterator<Item = &Usage> {
        self.0.iter()
    }

    pub fn into_iter(self) -> std::vec::IntoIter<Usage> {
        self.0.into_iter()
    }

    pub fn par_iter(&self) -> rayon::slice::Iter<'_, Usage> {
        self.0.par_iter()
    }

    pub fn get_line(&self, index: usize) -> Option<&str> {
        self.0.get(index).map(|usage| usage.line.as_str())
    }

    pub fn retain<F>(&mut self, f: F)
    where
        F: FnMut(&Usage) -> bool,
    {
        self.0.retain(f);
    }

    pub fn append(&mut self, other: Self) {
        let mut other_usages = other.0;
        self.0.append(&mut other_usages);
    }

    /// Prints the lines info to stdout.
    pub fn print(self) {
        let total = self.0.len();
        let (lines, indices) = self.deconstruct();
        utility::println_json_with_length!(total, lines, indices);
    }

    pub fn deconstruct(self) -> (Vec<String>, Vec<Vec<usize>>) {
        self.0
            .into_par_iter()
            .map(|usage| (usage.line, usage.indices))
            .unzip()
    }
}
