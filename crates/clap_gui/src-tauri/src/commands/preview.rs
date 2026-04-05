use serde::Serialize;
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

    let content = std::fs::read_to_string(&path)
        .map_err(|e| format!("Failed to read {}: {e}", path.display()))?;

    let all_lines: Vec<String> = content.lines().map(String::from).collect();
    let total = all_lines.len();

    let (start, end) = if let Some(target) = line {
        let target = target.saturating_sub(1);
        let half = max_lines / 2;
        let start = target.saturating_sub(half);
        let end = (start + max_lines).min(total);
        (start, end)
    } else {
        (0, max_lines.min(total))
    };

    let language = path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("")
        .to_string();

    Ok(PreviewContent {
        lines: all_lines[start..end].to_vec(),
        start_line: start + 1,
        language,
    })
}
