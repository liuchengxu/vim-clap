use grep_regex::{RegexMatcher, RegexMatcherBuilder};
use std::collections::HashMap;
use std::ops::Range;
use types::{Score, WordTerm};

/// A matcher for matching multiple words (OR).
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

    pub fn find_all_matches_range(&self, line: &str) -> Vec<(Range<usize>, usize)> {
        use grep_matcher::Matcher;

        let mut match_start_indices = vec![];

        self.matchers.iter().for_each(|(word_term, word_matcher)| {
            let _ = word_matcher.find_iter(line.as_bytes(), |matched| {
                match_start_indices.push((
                    Range {
                        start: matched.start(),
                        end: matched.end(),
                    },
                    word_term.text.len(),
                ));

                true
            });
        });

        match_start_indices
    }

    pub fn find_keyword_matches(
        &self,
        line: &str,
        keyword_highlights: &HashMap<String, String>,
    ) -> Vec<(Range<usize>, usize, String)> {
        use grep_matcher::Matcher;

        let mut match_start_indices = vec![];

        self.matchers.iter().for_each(|(word_term, word_matcher)| {
            let _ = word_matcher.find_iter(line.as_bytes(), |matched| {
                match_start_indices.push((
                    Range {
                        start: matched.start(),
                        end: matched.end(),
                    },
                    word_term.text.len(),
                    keyword_highlights
                        .get(&word_term.text)
                        .cloned()
                        .unwrap_or_default(),
                ));

                true
            });
        });

        match_start_indices
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_all_matches_start() {
        let word_matcher = WordMatcher::new(vec!["world".to_string().into()]);
        let line = "hello world world";
        assert_eq!(
            vec![
                (Range { start: 6, end: 11 }, 5),
                (Range { start: 12, end: 17 }, 5)
            ],
            word_matcher.find_all_matches_range(line)
        );
    }
}
