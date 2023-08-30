use crate::{Code, Diagnostic, PartialSpan};
use lsp_types::DiagnosticSeverity;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;
use std::process::Stdio;

#[derive(Serialize, Deserialize, Debug)]
struct CargoCheckErrorMessage {
    code: Code,
    level: String,
    message: String,
    spans: Vec<PartialSpan>,
}

pub struct RustLinter<'a, P> {
    pub source_file: P,
    pub workspace: &'a Path,
}

#[derive(Debug, Clone)]
pub enum LinterSource {
    CargoCheck,
    CargoClippy,
}

#[derive(Debug, Clone)]
pub struct LinterResult {
    pub source: LinterSource,
    pub diagonostics: Vec<Diagnostic>,
}

impl<'a, P: AsRef<Path>> RustLinter<'a, P> {
    pub fn cargo_check(&self) -> std::io::Result<Vec<Diagnostic>> {
        let output = std::process::Command::new("cargo")
            .args(["check", "--frozen", "--message-format=json", "-q"])
            // .stderr(Stdio::null())
            .current_dir(self.workspace)
            .output()
            .expect("Failed to run cargo check");

        Ok(self.parse_cargo_message(&output.stdout))
    }

    pub fn cargo_clippy(&self) -> std::io::Result<Vec<Diagnostic>> {
        let output = std::process::Command::new("cargo")
            .args([
                "clippy",
                "--message-format=json",
                "--all-features",
                "--all-targets",
                "--manifest-path",
                "Cargo.toml",
                "--",
                "-D",
                "warnings",
            ])
            .current_dir(self.workspace)
            .output()?;

        Ok(self.parse_cargo_message(&output.stdout))
    }

    fn parse_cargo_message(&self, stdout: &[u8]) -> Vec<Diagnostic> {
        let stdout = String::from_utf8_lossy(stdout);

        let source_filename = self
            .source_file
            .as_ref()
            .strip_prefix(self.workspace.parent().unwrap_or(self.workspace))
            .unwrap_or(self.source_file.as_ref())
            .to_str()
            .expect("Filename must exist");

        let mut diagonostics = Vec::new();

        for line in stdout.split('\n') {
            if !line.is_empty() {
                let line: HashMap<String, Value> = serde_json::from_str(line).unwrap();

                if let Some(error) = line.get("message") {
                    let error_message: CargoCheckErrorMessage =
                        match serde_json::from_value(error.clone()).ok() {
                            Some(v) => v,
                            None => {
                                continue;
                            }
                        };

                    let CargoCheckErrorMessage {
                        code,
                        level,
                        message,
                        spans,
                    } = error_message;

                    let severity = match level.as_str() {
                        "warning" => Some(DiagnosticSeverity::WARNING),
                        _ => None,
                    };

                    if !spans.is_empty() {
                        for span in spans {
                            tracing::debug!(
                                "======== file_name: {}, source_file: {}",
                                span.file_name,
                                source_filename
                            );
                            if span.file_name == source_filename {
                                let diagonostic = Diagnostic {
                                    line_start: span.line_start,
                                    line_end: span.line_end,
                                    column_start: span.column_start,
                                    column_end: span.column_end,
                                    code: code.clone(),
                                    severity,
                                    message: message.clone(),
                                };
                                diagonostics.push(diagonostic);
                            }
                        }
                    }
                }
            }
        }

        diagonostics
    }
}
