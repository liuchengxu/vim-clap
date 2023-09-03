mod linters;

use lsp_types::DiagnosticSeverity;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::task::JoinHandle;

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

#[derive(Debug, Clone)]
pub enum LintEngine {
    Gopls,
    RustCargoCheck,
    RustCargoClippy,
    ShellCheck,
    Vint,
}

#[derive(Debug, Clone)]
pub struct LintResult {
    pub engine: LintEngine,
    pub diagnostics: Vec<Diagnostic>,
}

pub trait HandleLintResult {
    fn handle_lint_result(&self, lint_result: LintResult) -> std::io::Result<()>;
}

#[derive(Debug, Clone)]
enum WorkspaceFinder {
    RootMarkers(&'static [&'static str]),
    /// Use the parent directory as the workspace if no explicit root markers.
    ParentOfSourceFile,
}

impl WorkspaceFinder {
    fn find_workspace<'a>(&'a self, source_file: &'a Path) -> Option<&Path> {
        match self {
            Self::RootMarkers(root_markers) => paths::find_project_root(source_file, root_markers),
            Self::ParentOfSourceFile => Some(source_file.parent().unwrap_or(source_file)),
        }
    }
}

/// Returns the working directory for running the command of lint engine.
pub fn find_workspace<'a>(filetype: impl AsRef<str>, source_file: &'a Path) -> Option<&'a Path> {
    static WORKSPACE_FINDERS: Lazy<HashMap<&str, WorkspaceFinder>> = Lazy::new(|| {
        HashMap::from_iter([
            ("rust", WorkspaceFinder::RootMarkers(&["Cargo.toml"])),
            ("go", WorkspaceFinder::RootMarkers(&["go.mod", ".git"])),
            ("sh", WorkspaceFinder::ParentOfSourceFile),
            ("vim", WorkspaceFinder::ParentOfSourceFile),
        ])
    });

    WORKSPACE_FINDERS
        .get(filetype.as_ref())
        .and_then(|workspace_finder| workspace_finder.find_workspace(&source_file))
}

pub fn lint_in_background<Handler: HandleLintResult + Send + Sync + Clone + 'static>(
    source_file: PathBuf,
    workspace: &Path,
    handler: Handler,
) -> std::io::Result<Option<Vec<JoinHandle<()>>>> {
    if let Some(ext) = source_file.extension().and_then(|s| s.to_str()) {
        let diagnostics = match ext {
            "rs" => {
                let linter = linters::rust::RustLinter {
                    source_file,
                    workspace: workspace.to_path_buf(),
                };

                return Ok(Some(linter.start(handler)));
            }
            "sh" => linters::sh::lint_shell_script(&source_file, workspace)?,
            "go" => linters::go::start_gopls(&source_file, workspace)?,
            "vim" => linters::vim::start_vint(&source_file, workspace)?,
            _ => {
                return Ok(None);
            }
        };

        let _ = handler.handle_lint_result(LintResult {
            engine: LintEngine::ShellCheck,
            diagnostics,
        });
    }

    Ok(None)
}

pub fn lint_file(
    source_file: impl AsRef<Path>,
    workspace: &Path,
) -> std::io::Result<Vec<Diagnostic>> {
    let linter = linters::rust::RustLinter {
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
