mod search_engine;

pub use self::search_engine::ctags::Readtags;
pub use self::search_engine::regex::RegexSearcher;

/// All the lines as well as their match indices that can be sent to the vim side directly.
#[derive(Clone, Debug, Default)]
pub struct UsagesInfo {
    pub lines: Vec<String>,
    pub indices: Vec<Vec<usize>>,
}

impl UsagesInfo {
    /// Constructs a new instance of [`UsagesInfo`].
    pub fn new(lines: Vec<String>, indices: Vec<Vec<usize>>) -> Self {
        Self { lines, indices }
    }

    /// Prints the lines info to stdout.
    pub fn print(&self) {
        let total = self.lines.len();
        let Self { lines, indices } = self;
        utility::println_json_with_length!(total, lines, indices);
    }

    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }
}
