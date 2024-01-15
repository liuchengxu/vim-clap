use crate::linting::{
    Code, Diagnostic, DiagnosticSpan, HandleLinterDiagnostics, LintEngine, LinterDiagnostics,
    RustLintEngine, Severity,
};
use serde::Deserialize;
use std::path::PathBuf;
use std::process::Stdio;
use tokio::task::JoinHandle;

#[derive(Clone)]
pub struct RustLinter {
    source_file: PathBuf,
    workspace_root: PathBuf,
}

impl RustLinter {
    pub fn new(source_file: PathBuf, workspace_root: PathBuf) -> Self {
        Self {
            source_file,
            workspace_root,
        }
    }

    pub fn run<Handler: HandleLinterDiagnostics + Send + Sync + Clone + 'static>(
        self,
        handler: Handler,
    ) -> Vec<JoinHandle<()>> {
        let mut handles = Vec::with_capacity(2);
        let worker = tokio::task::spawn_blocking({
            let handler = handler.clone();
            let linter = self.clone();

            move || {
                if let Ok(linter_result) = linter.cargo_check() {
                    let _ = handler.handle_linter_result(linter_result);
                }
            }
        });
        handles.push(worker);

        let worker = tokio::task::spawn_blocking({
            let linter = self;

            move || {
                if let Ok(linter_result) = linter.cargo_clippy() {
                    let _ = handler.handle_linter_result(linter_result);
                }
            }
        });
        handles.push(worker);

        handles
    }

    pub fn start(self, diagnostics_sender: tokio::sync::mpsc::UnboundedSender<LinterDiagnostics>) {
        tokio::task::spawn_blocking({
            let diagnostics_sender = diagnostics_sender.clone();
            let linter = self.clone();

            move || {
                if let Ok(linter_result) = linter.cargo_check() {
                    let _ = diagnostics_sender.send(linter_result);
                }
            }
        });

        tokio::task::spawn_blocking({
            let linter = self;

            move || {
                if let Ok(linter_result) = linter.cargo_clippy() {
                    let _ = diagnostics_sender.send(linter_result);
                }
            }
        });
    }

    pub fn cargo_check(&self) -> std::io::Result<LinterDiagnostics> {
        let output = std::process::Command::new("cargo")
            .args(["check", "--frozen", "--message-format=json", "-q"])
            .stderr(Stdio::null())
            .current_dir(&self.workspace_root)
            .output()?;

        Ok(LinterDiagnostics {
            engine: LintEngine::Rust(RustLintEngine::CargoCheck),
            diagnostics: self.parse_cargo_message(&output.stdout),
        })
    }

    fn cargo_clippy(&self) -> std::io::Result<LinterDiagnostics> {
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
            .current_dir(&self.workspace_root)
            .output()?;

        Ok(LinterDiagnostics {
            engine: LintEngine::Rust(RustLintEngine::CargoClippy),
            diagnostics: self.parse_cargo_message(&output.stdout),
        })
    }

    fn parse_cargo_message(&self, stdout: &[u8]) -> Vec<Diagnostic> {
        let Some(source_filename) = self
            .source_file
            .strip_prefix(self.workspace_root.parent().unwrap_or(&self.workspace_root))
            .unwrap_or(self.source_file.as_ref())
            .to_str()
        else {
            return Vec::new();
        };

        stdout
            .split(|&b| b == b'\n')
            .map(|line| line.strip_suffix(b"\r").unwrap_or(line))
            .filter_map(process_line)
            .filter_map(|cargo_message| match cargo_message {
                CargoMessage::Diagnostic(diagonostic) => {
                    process_cargo_diagnostic(diagonostic, source_filename)
                }
            })
            .collect()
    }
}

/// Filter out the diagnostics specific to `source_filename` and convert it to [`Diagnostic`].
///
/// NOTE: The diagnostic with empty spans will be discarded.
fn process_cargo_diagnostic(
    cargo_diagnostic: cargo_metadata::diagnostic::Diagnostic,
    source_filename: &str,
) -> Option<Diagnostic> {
    use cargo_metadata::diagnostic::DiagnosticLevel;

    let severity = match cargo_diagnostic.level {
        DiagnosticLevel::Error | DiagnosticLevel::Ice => Severity::Error,
        DiagnosticLevel::Warning | DiagnosticLevel::FailureNote => Severity::Warning,
        DiagnosticLevel::Note => Severity::Note,
        DiagnosticLevel::Help => Severity::Help,
        _ => Severity::Unknown,
    };

    let code = cargo_diagnostic
        .code
        .map(|c| Code { code: c.code })
        .unwrap_or_default();

    // Ignore the diagnostics without span.
    if cargo_diagnostic.spans.is_empty() {
        return None;
    }

    let mut primary_span_label = String::default();
    let mut suggested_replacement = None;

    let spans = cargo_diagnostic
        .spans
        .iter()
        .filter_map(|span| {
            if span.file_name == source_filename {
                if let (true, Some(label)) = (span.is_primary, &span.label) {
                    primary_span_label.push_str(label);
                    if let Some(suggestion) = &span.suggested_replacement {
                        let message = format!("{} `{suggestion}`", cargo_diagnostic.message);
                        suggested_replacement.replace(message);
                    }
                }
                Some(DiagnosticSpan {
                    line_start: span.line_start,
                    line_end: span.line_end,
                    column_start: span.column_start,
                    column_end: span.column_end,
                })
            } else {
                None
            }
        })
        .collect::<Vec<_>>();

    cargo_diagnostic.children.iter().for_each(|diagnostic| {
        // Stop at the first suggested_replacement.
        let _ = diagnostic.spans.iter().try_for_each(|span| {
            if span.file_name == source_filename {
                if let (true, Some(suggestion)) = (span.is_primary, &span.suggested_replacement) {
                    let message = format!("{} `{suggestion}`", diagnostic.message);
                    suggested_replacement.replace(message);
                    return Err(());
                }
            }
            Ok(())
        });
    });

    if spans.is_empty() {
        return None;
    }

    // Enrich the display message by merging the potential primary span label.
    let mut message = cargo_diagnostic.message;
    if !primary_span_label.is_empty() {
        message.push_str(", ");
        message.push_str(&primary_span_label);
    }

    if let Some(suggestion) = suggested_replacement {
        message.push_str(", ");
        message.push_str(&suggestion);
    }

    Some(Diagnostic {
        spans,
        code,
        severity,
        message,
    })
}

enum CargoMessage {
    // CompilerArtifact(cargo_metadata::Artifact),
    Diagnostic(cargo_metadata::diagnostic::Diagnostic),
}

#[derive(Deserialize)]
#[serde(untagged)]
enum JsonMessage {
    Cargo(cargo_metadata::Message),
    Rustc(cargo_metadata::diagnostic::Diagnostic),
}

// https://github.com/rust-lang/rust-analyzer/blob/12e28c35758051dd6bc9cdf419a50dff80fab64d/crates/flycheck/src/lib.rs#L483
// Try to deserialize a message from Cargo or Rustc.
#[allow(clippy::single_match)]
fn process_line(line: &[u8]) -> Option<CargoMessage> {
    let mut deserializer = serde_json::Deserializer::from_slice(line);
    deserializer.disable_recursion_limit();
    if let Ok(message) = JsonMessage::deserialize(&mut deserializer) {
        match message {
            // Skip certain kinds of messages to only spend time on what's useful
            JsonMessage::Cargo(message) => match message {
                // CompilerArtifact can be used to report the progress, which is useless on our end.
                // cargo_metadata::Message::CompilerArtifact(artifact) if !artifact.fresh => {
                // return Some(CargoMessage::CompilerArtifact(artifact));
                // }
                cargo_metadata::Message::CompilerMessage(msg) => {
                    return Some(CargoMessage::Diagnostic(msg.message));
                }
                _ => (),
            },
            JsonMessage::Rustc(message) => {
                return Some(CargoMessage::Diagnostic(message));
            }
        }
    }
    None
}
