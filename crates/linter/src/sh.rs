use crate::{Code, Diagnostic};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warning,
    Info,
    Style,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShellCheckMessage {
    pub file: String,
    pub line: usize,
    pub end_line: usize,
    pub column: usize,
    pub end_column: usize,
    pub level: Severity,
    pub code: usize,
    pub message: String,
    // pub fix: Option<Vec<Replacement>>
}

impl ShellCheckMessage {
    fn into_diagnostic(self) -> Diagnostic {
        Diagnostic {
            line_start: self.line,
            line_end: self.end_line,
            column_start: self.column,
            column_end: self.end_column,
            code: Code {
                code: Default::default(),
                explanation: None,
            },
            severity: None,
            message: self.message,
        }
    }
}

pub fn lint_shell_script(script_file: &Path, workspace: &Path) -> std::io::Result<Vec<Diagnostic>> {
    let output = std::process::Command::new("shellcheck")
        .arg("--format=json")
        .arg(script_file)
        .current_dir(workspace)
        .output()?;

    if let Ok(messages) = serde_json::from_slice::<Vec<ShellCheckMessage>>(&output.stdout) {
        let diagnostics = messages.into_iter().map(|m| m.into_diagnostic()).collect();
        return Ok(diagnostics);
    }

    Ok(Vec::new())
}
