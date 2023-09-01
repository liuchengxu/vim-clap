use crate::{Code, Diagnostic, HandleLintResult, LintEngine, LintResult, PartialSpan, Severity};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::task::JoinHandle;

#[derive(Serialize, Deserialize, Debug)]
struct CargoCheckErrorMessage {
    code: Code,
    level: String,
    message: String,
    spans: Vec<PartialSpan>,
}

#[derive(Clone)]
pub struct RustLinter {
    pub source_file: PathBuf,
    pub workspace: PathBuf,
}

impl RustLinter {
    pub fn start<Handler: HandleLintResult + Send + Sync + Clone + 'static>(
        self,
        handler: Handler,
    ) -> Vec<JoinHandle<()>> {
        let mut handles = Vec::with_capacity(2);
        let worker = tokio::task::spawn_blocking({
            let handler = handler.clone();
            let linter = self.clone();

            move || {
                if let Ok(lint_result) = linter.cargo_check() {
                    let _ = handler.handle_lint_result(lint_result);
                }
            }
        });
        handles.push(worker);

        let worker = tokio::task::spawn_blocking({
            let linter = self;

            move || {
                if let Ok(lint_result) = linter.cargo_clippy() {
                    let _ = handler.handle_lint_result(lint_result);
                }
            }
        });
        handles.push(worker);

        handles
    }

    pub fn cargo_check(&self) -> std::io::Result<LintResult> {
        let output = std::process::Command::new("cargo")
            .args(["check", "--frozen", "--message-format=json", "-q"])
            // .stderr(Stdio::null())
            .current_dir(&self.workspace)
            .output()?;

        let diagnostics = self.parse_cargo_message(&output.stdout);

        tracing::debug!("========= [cargo_check] diagnostics: {diagnostics:?}");

        Ok(LintResult {
            engine: LintEngine::RustCargoCheck,
            diagnostics,
        })
    }

    pub fn cargo_clippy(&self) -> std::io::Result<LintResult> {
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
            .current_dir(&self.workspace)
            .output()?;

        let diagnostics = self.parse_cargo_message(&output.stdout);

        Ok(LintResult {
            engine: LintEngine::RustCargoClippy,
            diagnostics,
        })
    }

    fn parse_cargo_message(&self, stdout: &[u8]) -> Vec<Diagnostic> {
        let stdout = String::from_utf8_lossy(stdout);

        let source_filename = self
            .source_file
            .strip_prefix(self.workspace.parent().unwrap_or(&self.workspace))
            .unwrap_or(self.source_file.as_ref())
            .to_str()
            .expect("source_filename must not contain invalid unicode");

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
                        "error" => Severity::Error,
                        "warning" => Severity::Warning,
                        _ => Severity::Unknown,
                    };

                    if !spans.is_empty() {
                        for span in spans {
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
