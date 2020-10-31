use std::collections::HashMap;

/// Line number of Vim is 1-based.
pub type VimLineNumber = usize;

/// Map of truncated line number to original full line.
///
/// Can't use HashMap<String, String> since we can't tell the original lines in the following case:
///
/// //  ..{ version = "1.0", features = ["derive"] }
/// //  ..{ version = "1.0", features = ["derive"] }
/// //  ..{ version = "1.0", features = ["derive"] }
/// //  ..{ version = "1.0", features = ["derive"] }
///
pub type LinesTruncatedMap = HashMap<VimLineNumber, String>;

/// Item for Source (display line and filter string)
#[derive(Debug)]
pub struct SourceItem {
    pub display: String,
    pub filter: Option<String>,
}

impl SourceItem {
    pub fn new(string: String) -> Self {
        SourceItem { display: string, filter: None }
    }
}

/// Tuple of (matched line text, filtering score, indices of matched elements)
pub type FilterResult = (SourceItem, i64, Vec<usize>);

