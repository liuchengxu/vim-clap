use lsp_types::{Position, TextEdit};
use similar::{ChangeTag, TextDiff};
use std::path::{Path, PathBuf};

pub async fn do_format(filetype: &str, source_file: PathBuf, workspace_root: &Path) {
    match filetype {
        "rust" => if let Ok(formatted) = run_rustfmt(&source_file, workspace_root).await {},
        _ => {}
    }
}

async fn run_rustfmt(source_file: &Path, workspace_root: &Path) -> std::io::Result<Vec<u8>> {
    let output = tokio::process::Command::new("rustfmt")
        .arg("--edition=2021")
        .arg("--emit=stdout")
        .arg(source_file)
        .current_dir(workspace_root)
        .output()
        .await?;

    Ok(output.stdout)
}

fn position_to_offset(lines: &[String], position: &Position) -> usize {
    if lines.is_empty() {
        return 0;
    }

    let line = std::cmp::min(position.line as usize, lines.len() - 1);
    let character = std::cmp::min(position.character as usize, lines[line].len());

    let chars_above: usize = lines[..line].iter().map(|text| text.len() + 1).sum();
    chars_above + character
}

fn offset_to_position(lines: &[String], offset: usize) -> Position {
    if lines.is_empty() {
        return Position::new(0, 0);
    }

    let mut offset = offset;
    for (line, text) in lines.iter().enumerate() {
        if offset <= text.len() {
            return Position::new(line as u32, offset as u32);
        }

        offset -= text.len() + 1;
    }

    let last_line = lines.len() - 1;
    let last_character = lines[last_line].len();
    Position::new(last_line as u32, last_character as u32)
}

pub fn apply_text_edits(
    lines: &[String],
    edits: &[TextEdit],
    position: &Position,
) -> anyhow::Result<(Vec<String>, Position)> {
    // Edits are ordered from bottom to top, from right to left.
    let mut edits_by_index = Vec::with_capacity(edits.len());
    for edit in edits {
        let start_line = edit.range.start.line as usize;
        let start_character = edit.range.start.character as usize;
        let end_line = edit.range.end.line as usize;
        let end_character = edit.range.end.character as usize;
        // let end_character: usize = edit.range.end.character.to_usize()?;

        let start = lines[..start_line.min(lines.len())]
            .iter()
            .map(String::len)
            .fold(0, |acc, l| acc + l + 1 /*line ending*/)
            + start_character;
        let end = lines[..end_line.min(lines.len())]
            .iter()
            .map(String::len)
            .fold(0, |acc, l| acc + l + 1 /*line ending*/)
            + end_character;
        edits_by_index.push((start, end, &edit.new_text));
    }

    let mut text = lines.join("\n");
    let mut offset = position_to_offset(lines, position);
    for (start, end, new_text) in edits_by_index {
        let start = start.min(text.len());
        let end = end.min(text.len());
        text = String::new() + &text[..start] + new_text + &text[end..];

        // Update offset only if the edit's entire range is before it.
        // Edits after the offset do not affect it.
        // Edits covering the offset cause unpredictable effect.
        if end <= offset {
            offset += new_text.len();
            offset -= new_text.matches("\r\n").count(); // line ending is counted as one offset
            offset -= offset.min(end - start);
        }
    }

    offset = offset.min(text.len());

    let new_lines: Vec<String> = text.lines().map(ToOwned::to_owned).collect();
    let new_position = offset_to_position(&new_lines, offset);
    tracing::debug!(
        "Position change after applying text edits: {:?} -> {:?}",
        position,
        new_position
    );

    Ok((new_lines, new_position))
}

// 1. run the command
// 2. parse the output and get the formatted content in diff.
// 3. apply the formatted content to the source file
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn find_newline_position(vec_u8: &[u8], n: usize) -> Option<usize> {
        let mut newline_count = 0;
        for (index, &byte) in vec_u8.iter().enumerate() {
            if byte == b'\n' {
                newline_count += 1;
                if newline_count == n {
                    return Some(index);
                }
            }
        }
        None
    }

    #[tokio::test]
    async fn test_formatter() {
        let source_file = PathBuf::from("/Users/xuliucheng/.vim/plugged/vim-clap/unformatted.rs");
        let workspace_root = PathBuf::from("/Users/xuliucheng/.vim/plugged/vim-clap");
        let original_file = std::fs::read(&source_file).unwrap();
        let formatted_file = run_rustfmt(&source_file, &workspace_root).await.unwrap();
        let pos = find_newline_position(&formatted_file, 3).unwrap();
        let (_, trimmed_formatted) = formatted_file.split_at(pos + 1);
        println!(
            "-------- formatted_file: \n{}",
            String::from_utf8_lossy(&trimmed_formatted)
        );
        let trimmed_formatted = trimmed_formatted.to_vec();
        let diff = TextDiff::from_lines(&original_file, &trimmed_formatted);

        // pub struct TextEdit {
        // /// The range of the text document to be manipulated. To insert
        // /// text into a document create a range where start === end.
        // pub range: Range,
        // /// The string to be inserted. For delete operations use an
        // /// empty string.
        // pub new_text: String,
        // }

        for op in diff.ops() {
            println!("============ op: {op:?}");
        }

        for (op, change) in diff.ops().iter().zip(diff.iter_all_changes()) {
            match change.tag() {
                ChangeTag::Equal => {
                    // Do nothing.
                }
                ChangeTag::Delete => {
                    println!("============ Delete op: {op:?}");
                    println!(
                        "---- delete: {change:?}, value: {}",
                        String::from_utf8_lossy(change.value())
                    );
                }
                ChangeTag::Insert => {
                    println!("============ Insert op: {op:?}");
                    println!(
                        "---- insert: {change:?}, value: {}",
                        String::from_utf8_lossy(change.value())
                    );
                }
            }
        }
    }
}
