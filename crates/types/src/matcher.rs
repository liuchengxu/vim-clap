/// Score of base matching algorithm(fzy, skim, etc).
pub type Score = i32;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum RankCriterion {
    /// Matching score.
    Score,
    /// Char index of first matched item.
    Begin,
    /// Char index of last matched item.
    End,
    /// Length of raw text.
    Length,
    NegativeScore,
    NegativeBegin,
    NegativeEnd,
    NegativeLength,
}

pub fn parse_criteria(text: &str) -> Option<RankCriterion> {
    match text.to_lowercase().as_ref() {
        "score" => Some(RankCriterion::Score),
        "begin" => Some(RankCriterion::Begin),
        "end" => Some(RankCriterion::End),
        "length" => Some(RankCriterion::Length),
        "-score" => Some(RankCriterion::NegativeScore),
        "-begin" => Some(RankCriterion::NegativeBegin),
        "-end" => Some(RankCriterion::NegativeEnd),
        "-length" => Some(RankCriterion::NegativeLength),
        _ => None,
    }
}

/// The greater, the better.
pub type Rank = [Score; 4];

#[derive(Debug, Clone)]
pub struct RankCalculator {
    criteria: Vec<RankCriterion>,
}

impl Default for RankCalculator {
    fn default() -> Self {
        Self {
            criteria: vec![
                RankCriterion::Score,
                RankCriterion::NegativeBegin,
                RankCriterion::NegativeEnd,
                RankCriterion::NegativeLength,
            ],
        }
    }
}

impl RankCalculator {
    pub fn new(mut criteria: Vec<RankCriterion>) -> Self {
        if !criteria.contains(&RankCriterion::Score)
            && !criteria.contains(&RankCriterion::NegativeScore)
        {
            criteria.insert(0, RankCriterion::Score);
        }
        criteria.dedup();
        criteria.truncate(4);
        Self { criteria }
    }

    /// Sort criteria for [`MatchedItem`], the greater the better.
    pub fn calculate_rank(&self, score: Score, begin: usize, end: usize, length: usize) -> Rank {
        let mut rank = [0; 4];
        let begin = begin as i32;
        let end = end as i32;
        let length = length as i32;

        for (index, criterion) in self.criteria.iter().enumerate() {
            let value = match criterion {
                RankCriterion::Score => score,
                RankCriterion::Begin => begin,
                RankCriterion::End => end,
                RankCriterion::Length => length,
                RankCriterion::NegativeScore => -score,
                RankCriterion::NegativeBegin => -begin,
                RankCriterion::NegativeEnd => -end,
                RankCriterion::NegativeLength => -length,
            };

            rank[index] = value;
        }

        rank
    }
}

/// A tuple of (score, matched_indices) for the line has a match given the query string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchResult {
    pub score: Score,
    pub indices: Vec<usize>,
}

impl MatchResult {
    pub fn new(score: Score, indices: Vec<usize>) -> Self {
        Self { score, indices }
    }

    pub fn add_score(&mut self, score: Score) {
        self.score += score;
    }

    pub fn extend_indices(&mut self, indices: Vec<usize>) {
        self.indices.extend(indices);
        self.indices.sort_unstable();
        self.indices.dedup();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_rank_sort() {
        let rank_calculator = RankCalculator::default();

        let rank0 = rank_calculator.calculate_rank(99, 5, 10, 15);
        // The greater `score`, the higher the rank.
        let rank1 = rank_calculator.calculate_rank(100, 5, 10, 15);
        assert!(rank0 < rank1);

        // The smaller `begin`, the higher the rank.
        let rank2 = rank_calculator.calculate_rank(100, 8, 10, 15);
        assert!(rank1 > rank2);

        // The smaller `end`, the higher the rank.
        let rank3 = rank_calculator.calculate_rank(100, 8, 12, 15);
        assert!(rank2 > rank3);

        // The smaller `length`, the higher the rank.
        let rank4 = rank_calculator.calculate_rank(100, 8, 12, 17);
        assert!(rank3 > rank4);
    }
}
