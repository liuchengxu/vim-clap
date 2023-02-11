mod search_engine;

use matcher::{ExactMatcher, InverseMatcher};
use rayon::prelude::*;
use std::ops::{Index, IndexMut};
use types::{CaseMatching, ExactTerm, InverseTerm};

pub use self::search_engine::{CtagsSearcher, GtagsSearcher, QueryType, RegexSearcher};

/// Matcher for filtering out the unqualified usages earlier at the searching stage.
#[derive(Debug, Clone, Default)]
pub struct UsageMatcher {
    pub exact_matcher: ExactMatcher,
    pub inverse_matcher: InverseMatcher,
}

impl UsageMatcher {
    pub fn new(exact_terms: Vec<ExactTerm>, inverse_terms: Vec<InverseTerm>) -> Self {
        Self {
            exact_matcher: ExactMatcher::new(exact_terms, CaseMatching::Smart),
            inverse_matcher: InverseMatcher::new(inverse_terms),
        }
    }

    /// Returns the match indices of exact terms if given `line` passes all the checks.
    fn match_indices(&self, line: &str) -> Option<Vec<usize>> {
        match (
            self.exact_matcher.find_matches(line),
            self.inverse_matcher.match_any(line),
        ) {
            (Some((_, indices)), false) => Some(indices),
            _ => None,
        }
    }

    /// Returns `true` if the result of The results of applying `self`
    /// is a superset of applying `other` on the same source.
    pub fn is_superset(&self, other: &Self) -> bool {
        self.exact_matcher
            .exact_terms
            .iter()
            .zip(other.exact_matcher.exact_terms.iter())
            .all(|(local, other)| local.is_superset(other))
            && self
                .inverse_matcher
                .inverse_terms()
                .iter()
                .zip(other.inverse_matcher.inverse_terms().iter())
                .all(|(local, other)| local.is_superset(other))
    }

    pub fn match_jump_line(
        &self,
        (jump_line, mut indices): (String, Vec<usize>),
    ) -> Option<(String, Vec<usize>)> {
        if let Some(exact_indices) = self.match_indices(&jump_line) {
            indices.extend(exact_indices);
            indices.sort_unstable();
            indices.dedup();
            Some((jump_line, indices))
        } else {
            None
        }
    }
}

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

impl IntoIterator for Usages {
    type Item = Usage;
    type IntoIter = std::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
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
