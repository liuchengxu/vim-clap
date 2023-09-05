mod linters;

use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::task::JoinHandle;

#[derive(Serialize, Deserialize, Default, Debug, Clone, Eq, PartialEq)]
pub struct Code {
    pub code: String,
    // Ignore `explanation` as it is too verbose and nevery displayed.
    // pub explanation: Option<String>,
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
    #[serde(flatten)]
    pub code: Code,
    pub severity: Severity,
    pub message: String,
}

impl PartialEq for Diagnostic {
    fn eq(&self, other: &Self) -> bool {
        let is_same_code = || !self.code.code.is_empty() && self.code.code == other.code.code;

        // If two diagnostics point to the same location and have the
        // same message, they visually make no differences. For instance,
        // some linter does not provide the severity property but has the
        // rest fields as same as the other linters.
        self.line_start == other.line_start
            && self.column_start == other.column_start
            && self.column_end == other.column_end
            // Having two diagnostics with the same code but different message is possible, which
            // points to the same error essentially.
            && (is_same_code() || self.message == other.message)
    }
}

impl Eq for Diagnostic {}

impl Diagnostic {
    pub fn human_message(&self) -> String {
        format!("[{}] {}", self.code.code, self.message)
    }
}

#[derive(Debug, Clone)]
pub enum RustLintEngine {
    CargoCheck,
    CargoClippy,
}

#[derive(Debug, Clone)]
pub enum LintEngine {
    Gopls,
    Rust(RustLintEngine),
    ShellCheck,
    Typos,
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
pub fn find_workspace(filetype: impl AsRef<str>, source_file: &Path) -> Option<&Path> {
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
        .and_then(|workspace_finder| workspace_finder.find_workspace(source_file))
}

// source_file => Available Linters => Enabled Linters => Run

pub fn lint_in_background<Handler>(
    source_file: PathBuf,
    workspace: &Path,
    handler: Handler,
) -> std::io::Result<Option<Vec<JoinHandle<()>>>>
where
    Handler: HandleLintResult + Send + Sync + Clone + 'static,
{
    tokio::task::spawn_blocking({
        let handler = handler.clone();
        let source_file = source_file.clone();
        let workspace = workspace.to_path_buf();
        move || {
            if let Ok(diagnostics) = linters::typos::run_typos(&source_file, &workspace) {
                let _ = handler.handle_lint_result(LintResult {
                    engine: LintEngine::Typos,
                    diagnostics,
                });
            }
        }
    });

    if let Some(ext) = source_file.extension().and_then(|s| s.to_str()) {
        let diagnostics = match ext {
            "rs" => {
                return Ok(Some(
                    linters::rust::RustLinter::new(source_file, workspace.to_path_buf())
                        .run(handler),
                ));
            }
            "sh" => linters::sh::run_shellcheck(&source_file, workspace)?,
            "go" => linters::go::run_gopls(&source_file, workspace)?,
            "vim" => linters::vim::run_vint(&source_file, workspace)?,
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
    let linter =
        linters::rust::RustLinter::new(source_file.as_ref().to_path_buf(), workspace.to_path_buf());

    linter.cargo_check().map(|res| res.diagnostics)
}
