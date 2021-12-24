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
    pub word: String,
}

impl ExactTerm {
    pub fn new(ty: ExactTermType, word: String) -> Self {
        Self { ty, word }
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
    pub word: String,
}

impl InverseTerm {
    pub fn new(ty: InverseTermType, word: String) -> Self {
        Self { ty, word }
    }

    /// Returns true if the full line of given `item` matches the inverse term.
    pub fn match_full_line(&self, full_search_line: &str) -> bool {
        let query = self.word.as_str();
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
    pub word: String,
}

impl FuzzyTerm {
    pub fn new(ty: FuzzyTermType, word: String) -> Self {
        Self { ty, word }
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
    pub word: String,
}

impl SearchTerm {
    pub fn new(ty: TermType, word: String) -> Self {
        Self { ty, word }
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
        let (ty, word) = if let Some(stripped) = s.strip_prefix('\'') {
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
            word: word.into(),
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
