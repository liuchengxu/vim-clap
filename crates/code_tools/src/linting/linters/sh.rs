use crate::linting::{Code, Diagnostic, DiagnosticSpan, Linter, LinterDiagnostics};
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
#[allow(dead_code)]
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
            Severity::Error => crate::linting::Severity::Error,
            Severity::Warning => crate::linting::Severity::Warning,
            Severity::Info => crate::linting::Severity::Info,
            Severity::Style => crate::linting::Severity::Style,
        };
        Diagnostic {
            spans: vec![DiagnosticSpan {
                line_start: self.line,
                line_end: self.end_line,
                column_start: self.column,
                column_end: self.end_column,
            }],
            code: Code::default(),
            severity,
            message: self.message,
        }
    }
}

pub struct ShellCheck;

impl Linter for ShellCheck {
    const EXE: &'static str = "shellcheck";

    fn add_args(cmd: &mut tokio::process::Command, source_file: &Path) {
        cmd.arg("--format=json").arg(source_file);
    }

    async fn lint_file(
        &self,
        source_file: &Path,
        workspace_root: &Path,
    ) -> std::io::Result<LinterDiagnostics> {
        let mut cmd = Self::command(source_file, workspace_root)?;

        let output = cmd.output().await?;

        if let Ok(messages) = serde_json::from_slice::<Vec<ShellCheckMessage>>(&output.stdout) {
            let diagnostics = messages.into_iter().map(|m| m.into_diagnostic()).collect();
            return Ok(LinterDiagnostics {
                source: Self::EXE,
                diagnostics,
            });
        }

        Ok(LinterDiagnostics {
            source: Self::EXE,
            diagnostics: Vec::new(),
        })
    }
}
