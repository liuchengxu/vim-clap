use std::path::PathBuf;
use std::process::Command;
use tauri::State;

use crate::state::AppState;

#[tauri::command]
pub fn open_in_editor(
    path: String,
    line: Option<usize>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let editor = state
        .config
        .general
        .editor
        .clone()
        .or_else(|| std::env::var("EDITOR").ok())
        .or_else(|| std::env::var("VISUAL").ok());

    let path = PathBuf::from(&path);

    match editor {
        Some(editor) => {
            let mut cmd = Command::new(&editor);
            if let Some(line_num) = line {
                match editor.as_str() {
                    "code" | "code-insiders" => {
                        cmd.arg("--goto");
                        cmd.arg(format!("{}:{line_num}", path.display()));
                    }
                    _ => {
                        cmd.arg(format!("+{line_num}"));
                        cmd.arg(&path);
                    }
                };
            } else {
                cmd.arg(&path);
            }
            cmd.spawn()
                .map_err(|e| format!("Failed to open editor '{editor}': {e}"))?;
            Ok(())
        }
        None => {
            #[cfg(target_os = "macos")]
            {
                Command::new("open")
                    .arg(&path)
                    .spawn()
                    .map_err(|e| format!("Failed to open file: {e}"))?;
            }
            #[cfg(target_os = "linux")]
            {
                Command::new("xdg-open")
                    .arg(&path)
                    .spawn()
                    .map_err(|e| format!("Failed to open file: {e}"))?;
            }
            Ok(())
        }
    }
}
