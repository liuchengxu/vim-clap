use crate::{Code, Diagnostic, HandleLintResult, LintEngine, LintResult, RustLintEngine, Severity};
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use tokio::task::JoinHandle;

#[derive(Deserialize, Debug)]
pub struct PartialSpan {
    line_start: usize,
    line_end: usize,
    column_start: usize,
    column_end: usize,
    file_name: String,
    #[allow(unused)]
    label: Option<String>,
    #[allow(unused)]
    level: Option<String>,
    #[allow(unused)]
    rendered: Option<String>,
}

#[derive(Deserialize, Debug)]
struct CargoCheckErrorMessage {
    code: Code,
    level: String,
    message: String,
    spans: Vec<PartialSpan>,
}

#[derive(Clone)]
pub struct RustLinter {
    source_file: PathBuf,
    workspace: PathBuf,
}

impl RustLinter {
    pub fn new(source_file: PathBuf, workspace: PathBuf) -> Self {
        Self {
            source_file,
            workspace,
        }
    }

    pub fn run<Handler: HandleLintResult + Send + Sync + Clone + 'static>(
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
            .stderr(Stdio::null())
            .current_dir(&self.workspace)
            .output()?;

        Ok(LintResult {
            engine: LintEngine::Rust(RustLintEngine::CargoCheck),
            diagnostics: self.parse_cargo_message(&output.stdout),
        })
    }

    fn cargo_clippy(&self) -> std::io::Result<LintResult> {
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
            .stderr(Stdio::null())
            .current_dir(&self.workspace)
            .output()?;

        Ok(LintResult {
            engine: LintEngine::Rust(RustLintEngine::CargoClippy),
            diagnostics: self.parse_cargo_message(&output.stdout),
        })
    }

    fn parse_cargo_message(&self, stdout: &[u8]) -> Vec<Diagnostic> {
        let Some(source_filename) = self
            .source_file
            .strip_prefix(self.workspace.parent().unwrap_or(&self.workspace))
            .unwrap_or(self.source_file.as_ref())
            .to_str()
        else {
            return Vec::new();
        };

        let mut diagonostics = Vec::new();

        let lines = stdout
            .split(|&b| b == b'\n')
            .map(|line| line.strip_suffix(b"\r").unwrap_or(line));

        for line in lines {
            if let Ok(mut line) = serde_json::from_slice::<HashMap<String, Value>>(line) {
                if let Some(message) = line.remove("message") {
                    if let Ok(error_message) =
                        serde_json::from_value::<CargoCheckErrorMessage>(message)
                    {
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

                        let line_diagnostics = spans.into_iter().filter_map(|span| {
                            if span.file_name == source_filename {
                                Some(Diagnostic {
                                    line_start: span.line_start,
                                    line_end: span.line_end,
                                    column_start: span.column_start,
                                    column_end: span.column_end,
                                    code: code.clone(),
                                    severity,
                                    message: message.clone(),
                                })
                            } else {
                                None
                            }
                        });

                        diagonostics.extend(line_diagnostics);
                    }
                }
            }
        }

        diagonostics
    }
}
