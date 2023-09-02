mod go;
mod rust;
mod sh;

use lsp_types::DiagnosticSeverity;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tokio::task::JoinHandle;

#[derive(Debug)]
pub enum Linter {
    Rust,
}

#[derive(Serialize, Deserialize, Default, Debug, Clone, Eq, PartialEq)]
pub struct Code {
    pub code: String,
    pub explanation: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, Eq, PartialEq)]
pub enum Severity {
    Error,
    Warning,
    Info,
    Hint,
    Help,
    Style,
    Unknown,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Diagnostic {
    pub line_start: usize,
    pub line_end: usize,
    pub column_start: usize,
    pub column_end: usize,
    pub code: Code,
    pub severity: Severity,
    pub message: String,
}

impl PartialEq for Diagnostic {
    fn eq(&self, other: &Self) -> bool {
        // If two diagnostics point to the same location and have the
        // same message, they visually make no differences. For instance,
        // some linter does not provide the severity property but has the
        // rest fields as same as the other linters.
        self.line_start == other.line_start
            && self.column_start == other.column_start
            && self.column_end == other.column_end
            && self.message == other.message
    }
}

impl Eq for Diagnostic {}

impl Diagnostic {
    pub fn human_message(&self) -> String {
        format!("[{}] {}", self.code.code, self.message)
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct PartialSpan {
    line_start: usize,
    line_end: usize,
    column_start: usize,
    column_end: usize,
    file_name: String,
    label: Option<String>,
    level: Option<String>,
    rendered: Option<String>,
}

#[derive(Debug, Clone)]
pub enum LintEngine {
    Gopls,
    RustCargoCheck,
    RustCargoClippy,
    ShellCheck,
}

#[derive(Debug, Clone)]
pub struct LintResult {
    pub engine: LintEngine,
    pub diagnostics: Vec<Diagnostic>,
}

pub trait HandleLintResult {
    fn handle_lint_result(&self, lint_result: LintResult) -> std::io::Result<()>;
}

pub fn lint_in_background<Handler: HandleLintResult + Send + Sync + Clone + 'static>(
    source_file: PathBuf,
    workspace: &Path,
    handler: Handler,
) -> Vec<JoinHandle<()>> {
    if let Some(ext) = source_file.extension().and_then(|s| s.to_str()) {
        match ext {
            "rs" => {
                let linter = self::rust::RustLinter {
                    source_file,
                    workspace: workspace.to_path_buf(),
                };

                return linter.start(handler);
            }
            "sh" => {
                if let Ok(diagnostics) = self::sh::lint_shell_script(&source_file, workspace) {
                    let _ = handler.handle_lint_result(LintResult {
                        engine: LintEngine::ShellCheck,
                        diagnostics,
                    });
                }
            }
            "go" => {
                let diagnostics = self::go::start_gopls(&source_file, workspace).unwrap();

                let _ = handler.handle_lint_result(LintResult {
                    engine: LintEngine::Gopls,
                    diagnostics,
                });
            }
            _ => {}
        }
    }

    Vec::new()
}

pub fn lint_file(
    source_file: impl AsRef<Path>,
    workspace: &Path,
) -> std::io::Result<Vec<Diagnostic>> {
    let linter = self::rust::RustLinter {
        source_file: source_file.as_ref().to_path_buf(),
        workspace: workspace.to_path_buf(),
    };

    linter.cargo_check().map(|res| res.diagnostics)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cargo() {
        let source_file =
            Path::new("/Users/xuliucheng/.vim/plugged/vim-clap/crates/linter/src/lib.rs");

        let workspace = paths::find_project_root(source_file, &["Cargo.toml"]).unwrap();
        let diagonostics = lint_file(source_file, workspace);
        println!("======= diagonostics: {diagonostics:#?}");
    }
}
