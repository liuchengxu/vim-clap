mod rust;

use lsp_types::DiagnosticSeverity;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub enum Linter {
    Rust,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Code {
    pub code: String,
    pub explanation: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Diagnostic {
    pub line_start: usize,
    pub line_end: usize,
    pub column_start: usize,
    pub column_end: usize,
    pub code: Code,
    pub severity: Option<DiagnosticSeverity>,
    pub message: String,
}

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
    CargoCheck,
    CargoClippy,
}

// extensions => filetype => multiple linters

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
) {
    let linter = self::rust::RustLinter {
        source_file: source_file.clone(),
        workspace: workspace.to_path_buf(),
    };

    tokio::task::spawn_blocking({
        let handler = handler.clone();
        let linter = linter.clone();

        move || {
            if let Ok(lint_result) = linter.cargo_check() {
                let _ = handler.handle_lint_result(lint_result);
            }
        }
    });

    tokio::task::spawn_blocking({
        let linter = self::rust::RustLinter {
            source_file: source_file.to_path_buf(),
            workspace: workspace.to_path_buf(),
        };

        move || {
            if let Ok(lint_result) = linter.cargo_clippy() {
                let _ = handler.handle_lint_result(lint_result);
            }
        }
    });
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

    // let lint_result = linter.cargo_check()?;
    // let lint_result = linter.cargo_clippy()?;
    // handler.handle_lint_result(lint_result)?;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cargo() {
        let source_file =
            Path::new("/Users/xuliucheng/.vim/plugged/vim-clap/crates/linter/src/lib.rs");

        let workspace = paths::find_project_root(&source_file, &["Cargo.toml"]).unwrap();
        let diagonostics = lint_file(&source_file, workspace);
        println!("======= diagonostics: {diagonostics:#?}");
    }
}
