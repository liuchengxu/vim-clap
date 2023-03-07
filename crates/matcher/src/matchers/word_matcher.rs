use grep_regex::{RegexMatcher, RegexMatcherBuilder};
use types::{Score, WordTerm};

/// Word matching using the `RegexMatcher`.
#[derive(Debug, Clone, Default)]
pub struct WordMatcher {
    matchers: Vec<(WordTerm, RegexMatcher)>,
}

impl WordMatcher {
    pub fn new(word_terms: Vec<WordTerm>) -> Self {
        let matchers = word_terms
            .into_iter()
            .filter_map(|word_term| {
                RegexMatcherBuilder::default()
                    .word(true)
                    .build(&word_term.text)
                    .ok()
                    .map(|word_matcher| (word_term, word_matcher))
            })
            .collect();

        Self { matchers }
    }

    pub fn is_empty(&self) -> bool {
        self.matchers.is_empty()
    }

    pub fn find_matches(&self, line: &str) -> Option<(Score, Vec<usize>)> {
        use grep_matcher::Matcher;

        let mut score = Score::default();

        let byte_indices: Vec<_> = self
            .matchers
            .iter()
            .filter_map(|(word_term, word_matcher)| {
                word_matcher
                    .find_at(line.as_bytes(), 0)
                    .ok()
                    .flatten()
                    .map(|mat| {
                        let start = mat.start();
                        let end = mat.end();
                        score += word_term.score(start);
                        start..end
                    })
            })
            .flatten()
            .collect();

        // In order to be consistent with the other matchers which use char-positions, even all
        // char-positions will be converted to byte-positions before sending to Vim/Neovim in the end.
        let indices = line
            .char_indices()
            .enumerate()
            .filter_map(|(char_idx, (byte_idx, _char))| {
                if byte_indices.contains(&byte_idx) {
                    Some(char_idx)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();

        if !indices.is_empty() {
            Some((score, indices))
        } else {
            None
        }
    }

    pub fn find_all_matches_in_byte_indices(&self, line: &str) -> Option<Vec<usize>> {
        use grep_matcher::Matcher;

        let byte_indices: Vec<_> = self
            .matchers
            .iter()
            .filter_map(|(_word_term, word_matcher)| {
                word_matcher
                    .find_at(line.as_bytes(), 0)
                    .ok()
                    .flatten()
                    .map(|mat| {
                        let start = mat.start();
                        let end = mat.end();
                        start..end
                    })
            })
            .flatten()
            .collect();

        Some(byte_indices)
    }
}
