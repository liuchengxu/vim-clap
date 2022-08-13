mod search_engine;

use std::ops::{Index, IndexMut};

use rayon::prelude::*;

pub use self::search_engine::{CtagsSearcher, GtagsSearcher, QueryType, RegexSearcher};

#[derive(Clone, Debug, Default)]
pub struct Usage {
    /// Display line.
    pub line: String,
    /// Highlights of matched elements.
    pub indices: Vec<usize>,
}

impl From<AddressableUsage> for Usage {
    fn from(addressable_usage: AddressableUsage) -> Self {
        let AddressableUsage { line, indices, .. } = addressable_usage;
        Self { line, indices }
    }
}

impl Usage {
    pub fn new(line: String, indices: Vec<usize>) -> Self {
        Self { line, indices }
    }
}

/// [`Usage`] with some structured information.
#[derive(Clone, Debug, Default)]
pub struct AddressableUsage {
    pub line: String,
    pub indices: Vec<usize>,
    pub path: String,
    pub line_number: usize,
}

impl PartialEq for AddressableUsage {
    fn eq(&self, other: &Self) -> bool {
        // Equal if the path and lnum are the same.
        (&self.path, self.line_number) == (&other.path, other.line_number)
    }
}

impl Eq for AddressableUsage {}

/// All the lines as well as their match indices that can be sent to the vim side directly.
#[derive(Clone, Debug, Default)]
pub struct Usages(Vec<Usage>);

impl From<Vec<Usage>> for Usages {
    fn from(inner: Vec<Usage>) -> Self {
        Self(inner)
    }
}

impl From<Vec<AddressableUsage>> for Usages {
    fn from(inner: Vec<AddressableUsage>) -> Self {
        Self(inner.into_iter().map(Into::into).collect())
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
}
