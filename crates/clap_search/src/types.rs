use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct FileResult {
    pub path: String,
    pub display_path: String,
    pub icon: String,
    pub match_indices: Vec<usize>,
    pub score: i32,
}

#[derive(Debug, Clone, Serialize)]
pub struct GrepResultItem {
    pub path: String,
    pub line_number: u64,
    pub line_content: String,
    pub match_indices: Vec<usize>,
    pub score: i32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
pub enum SearchPayload {
    #[serde(rename = "files")]
    Files {
        results: Vec<FileResult>,
        total_matched: usize,
        total_processed: usize,
        finished: bool,
    },
    #[serde(rename = "grep")]
    Grep {
        results: Vec<GrepResultItem>,
        total_matched: usize,
        total_processed: usize,
        finished: bool,
    },
}
