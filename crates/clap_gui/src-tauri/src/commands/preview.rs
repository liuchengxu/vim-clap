use serde::Serialize;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize)]
pub struct PreviewContent {
    pub lines: Vec<String>,
    pub start_line: usize,
    pub language: String,
}

#[tauri::command]
pub async fn preview_file(
    path: String,
    line: Option<usize>,
    max_lines: Option<usize>,
) -> Result<PreviewContent, String> {
    let path = PathBuf::from(&path);
    let max_lines = max_lines.unwrap_or(50);

    let file = File::open(&path)
        .map_err(|e| format!("Failed to open {}: {e}", path.display()))?;
    let reader = BufReader::new(file);

    // Determine the window of lines to read
    let start = if let Some(target) = line {
        let target = target.saturating_sub(1); // 0-indexed
        let half = max_lines / 2;
        target.saturating_sub(half)
    } else {
        0
    };

    let lines: Vec<String> = reader
        .lines()
        .enumerate()
        .skip(start)
        .take(max_lines)
        .filter_map(|(_, l)| l.ok())
        .collect();

    let language = path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("")
        .to_string();

    Ok(PreviewContent {
        lines,
        start_line: start + 1, // 1-indexed for display
        language,
    })
}
