use crate::{Code, Diagnostic, LinterResult};
use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warning,
    Info,
    Style,
}

#[derive(Debug, Deserialize)]
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
        let severity = match &self.level {
            Severity::Error => crate::Severity::Error,
            Severity::Warning => crate::Severity::Warning,
            Severity::Info => crate::Severity::Info,
            Severity::Style => crate::Severity::Style,
        };
        Diagnostic {
            line_start: self.line,
            line_end: self.end_line,
            column_start: self.column,
            column_end: self.end_column,
            code: Code::default(),
            severity,
            message: self.message,
        }
    }
}

pub async fn run_shellcheck(script_file: &Path, workspace: &Path) -> std::io::Result<LinterResult> {
    let output = tokio::process::Command::new("shellcheck")
        .arg("--format=json")
        .arg(script_file)
        .current_dir(workspace)
        .output()
        .await?;

    if let Ok(messages) = serde_json::from_slice::<Vec<ShellCheckMessage>>(&output.stdout) {
        let diagnostics = messages.into_iter().map(|m| m.into_diagnostic()).collect();
        return Ok(LinterResult {
            engine: crate::LintEngine::ShellCheck,
            diagnostics,
        });
    }

    Ok(LinterResult {
        engine: crate::LintEngine::ShellCheck,
        diagnostics: Vec::new(),
    })
}
