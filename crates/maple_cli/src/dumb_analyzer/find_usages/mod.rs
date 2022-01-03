mod search_engine;

use rayon::prelude::*;

pub use self::search_engine::ctags::TagsSearcher;
pub use self::search_engine::regex::RegexSearcher;

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

/// All the lines as well as their match indices that can be sent to the vim side directly.
#[derive(Clone, Debug, Default)]
pub struct Usages(Vec<Usage>);

impl From<Vec<Usage>> for Usages {
    fn from(inner: Vec<Usage>) -> Self {
        Self(inner)
    }
}

impl Usages {
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
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
