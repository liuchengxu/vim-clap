use crate::Score;

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum ExactTermType {
    /// exact-match.
    ///
    /// `'wild`: Items that include wild.
    Exact,
    /// prefix-exact-match
    ///
    /// `^music`: Items that start with music.
    PrefixExact,
    /// suffix-exact-match
    ///
    /// `.mp3$`: Items that end with .mp3
    SuffixExact,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ExactTerm {
    pub ty: ExactTermType,
    pub text: String,
}

impl ExactTerm {
    pub fn new(ty: ExactTermType, text: String) -> Self {
        Self { ty, text }
    }

    /// Returns `true` if the result of The results of applying `self`
    /// is a superset of applying `other` on the same source.
    pub fn is_superset(&self, other: &Self) -> bool {
        use ExactTermType::*;

        match (&self.ty, &other.ty) {
            (Exact, Exact) | (PrefixExact, PrefixExact) | (SuffixExact, SuffixExact) => {
                // Comparing with `'hello`, `'he` has more results.
                other.text.starts_with(&self.text)
            }
            (Exact, PrefixExact) | (Exact, SuffixExact) => true,
            _ => false,
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum InverseTermType {
    /// inverse-exact-match
    ///
    /// `!fire`: Items that do not include fire
    InverseExact,
    /// inverse-prefix-exact-match
    ///
    /// `!^music`: Items that do not start with music
    InversePrefixExact,
    /// inverse-suffix-exact-match
    ///
    /// `!.mp3$`: Items that do not end with .mp3
    InverseSuffixExact,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct InverseTerm {
    pub ty: InverseTermType,
    pub text: String,
}

impl InverseTerm {
    pub fn new(ty: InverseTermType, text: String) -> Self {
        Self { ty, text }
    }

    /// Returns `true` if the result of The results of applying `self`
    /// is a superset of applying `other` on the same source.
    pub fn is_superset(&self, other: &Self) -> bool {
        use InverseTermType::*;

        // Comparing with `!hello`, `!he` has less results.
        // In order to have a superset results, `self.text` needs to be longer.

        match (&self.ty, &other.ty) {
            (InverseExact, InverseExact)
            | (InversePrefixExact, InversePrefixExact)
            | (InverseSuffixExact, InverseSuffixExact) => self.text.starts_with(&other.text),
            (InversePrefixExact, InverseExact) | (InverseSuffixExact, InverseExact) => true,
            _ => false,
        }
    }

    /// Returns true if the full line of given `item` matches the inverse term.
    pub fn is_match(&self, full_search_line: &str) -> bool {
        let query = self.text.as_str();
        let trimmed = full_search_line.trim();
        match self.ty {
            InverseTermType::InverseExact => trimmed.contains(query),
            InverseTermType::InversePrefixExact => trimmed.starts_with(query),
            InverseTermType::InverseSuffixExact => trimmed.ends_with(query),
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum FuzzyTermType {
    /// fuzzy-match.
    ///
    /// `sbtrkt`: Items that match sbtrkt.
    Fuzzy,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct FuzzyTerm {
    pub ty: FuzzyTermType,
    pub text: String,
}

impl FuzzyTerm {
    pub fn new(ty: FuzzyTermType, text: String) -> Self {
        Self { ty, text }
    }

    pub fn len(&self) -> usize {
        self.text.len()
    }

    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum TermType {
    /// Items that match in fuzzy.
    Fuzzy(FuzzyTermType),
    /// Items that match something.
    Exact(ExactTermType),
    /// Items that do not match something.
    Inverse(InverseTermType),
    /// Items that match a text.
    Word,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct WordTerm {
    pub text: String,
}

impl WordTerm {
    pub fn score(&self, match_start: usize) -> Score {
        (self.text.len() + 1024 / match_start.max(1))
            .try_into()
            .unwrap_or_default()
    }
}

impl TermType {
    pub fn is_inverse(&self) -> bool {
        matches!(self, Self::Inverse(_))
    }

    pub fn is_exact(&self) -> bool {
        matches!(self, Self::Exact(_))
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct SearchTerm {
    pub ty: TermType,
    pub text: String,
}

impl SearchTerm {
    pub fn new(ty: TermType, text: String) -> Self {
        Self { ty, text }
    }

    pub fn is_inverse_term(&self) -> bool {
        self.ty.is_inverse()
    }

    pub fn is_exact_term(&self) -> bool {
        self.ty.is_exact()
    }
}

impl From<&str> for SearchTerm {
    fn from(s: &str) -> Self {
        let (ty, text) = if let Some(stripped) = s.strip_prefix('"') {
            (TermType::Word, stripped)
        } else if let Some(stripped) = s.strip_prefix('\'') {
            (TermType::Exact(ExactTermType::Exact), stripped)
        } else if let Some(stripped) = s.strip_prefix('^') {
            (TermType::Exact(ExactTermType::PrefixExact), stripped)
        } else if let Some(stripped) = s.strip_prefix('!') {
            if let Some(double_stripped) = stripped.strip_prefix('^') {
                (
                    TermType::Inverse(InverseTermType::InversePrefixExact),
                    double_stripped,
                )
            } else if let Some(double_stripped) = stripped.strip_suffix('$') {
                (
                    TermType::Inverse(InverseTermType::InverseSuffixExact),
                    double_stripped,
                )
            } else {
                (TermType::Inverse(InverseTermType::InverseExact), stripped)
            }
        } else if let Some(stripped) = s.strip_suffix('$') {
            (TermType::Exact(ExactTermType::SuffixExact), stripped)
        } else {
            (TermType::Fuzzy(FuzzyTermType::Fuzzy), s)
        };

        Self {
            ty,
            text: text.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_term_should_work() {
        use TermType::*;
        let query = "aaa 'bbb ^ccc ddd$ !eee !'fff !^ggg !hhh$";
        let terms = query.split_whitespace().map(Into::into).collect::<Vec<_>>();

        let expected = vec![
            SearchTerm::new(Fuzzy(FuzzyTermType::Fuzzy), "aaa".into()),
            SearchTerm::new(Exact(ExactTermType::Exact), "bbb".into()),
            SearchTerm::new(Exact(ExactTermType::PrefixExact), "ccc".into()),
            SearchTerm::new(Exact(ExactTermType::SuffixExact), "ddd".into()),
            SearchTerm::new(Inverse(InverseTermType::InverseExact), "eee".into()),
            SearchTerm::new(Inverse(InverseTermType::InverseExact), "'fff".into()),
            SearchTerm::new(Inverse(InverseTermType::InversePrefixExact), "ggg".into()),
            SearchTerm::new(Inverse(InverseTermType::InverseSuffixExact), "hhh".into()),
        ];

        for (expected, got) in expected.iter().zip(terms.iter()) {
            assert_eq!(expected, got);
        }
    }
}
