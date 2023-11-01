mod linters;

use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::future::Future;
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
    Note,
    Help,
    Style,
    Unknown,
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Debug, Clone)]
pub struct DiagnosticSpan {
    /// 1-based.
    pub line_start: usize,
    /// 1-based.
    pub line_end: usize,
    /// 1-based. Character offset.
    pub column_start: usize,
    /// 1-based. Character offset
    pub column_end: usize,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Diagnostic {
    pub message: String,
    /// A list of source code spans this diagnostic is associated with.
    pub spans: Vec<DiagnosticSpan>,
    #[serde(flatten)]
    pub code: Code,
    pub severity: Severity,
}

impl PartialEq for Diagnostic {
    fn eq(&self, other: &Self) -> bool {
        let is_same_code = || !self.code.code.is_empty() && self.code.code == other.code.code;

        // If two diagnostics point to the same location and have the
        // same message, they visually make no differences. For instance,
        // some linter does not provide the severity property but has the
        // rest fields as same as the other linters.
        //
        // TODO: custom DiagnosticSpan PartialEq impl?
        // self.line_start == other.line_start
        // && self.column_start == other.column_start
        // && self.column_end == other.column_end

        self.spans == other.spans
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
pub struct LinterResult {
    pub engine: LintEngine,
    pub diagnostics: Vec<Diagnostic>,
}

pub trait HandleLinterResult {
    fn handle_linter_result(&self, linter_result: LinterResult) -> std::io::Result<()>;
}

#[derive(Debug, Clone)]
enum WorkspaceFinder {
    RootMarkers(&'static [&'static str]),
    /// Use the parent directory as the workspace_root if no explicit root markers.
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
    use WorkspaceFinder::{ParentOfSourceFile, RootMarkers};

    static WORKSPACE_FINDERS: Lazy<HashMap<&str, WorkspaceFinder>> = Lazy::new(|| {
        HashMap::from([
            ("go", RootMarkers(&["go.mod", ".git"])),
            ("rust", RootMarkers(&["Cargo.toml"])),
            ("sh", ParentOfSourceFile),
            ("vim", ParentOfSourceFile),
            ("markdown", ParentOfSourceFile),
        ])
    });

    WORKSPACE_FINDERS
        .get(filetype.as_ref())
        .and_then(|workspace_finder| workspace_finder.find_workspace(source_file))
}

// source_file => Available Linters => Enabled Linters => Run

pub fn lint_in_background<Handler>(
    filetype: &str,
    source_file: PathBuf,
    workspace_root: &Path,
    handler: Handler,
) -> Vec<JoinHandle<()>>
where
    Handler: HandleLinterResult + Send + Sync + Clone + 'static,
{
    let mut handles = Vec::new();

    handles.push(tokio::spawn({
        let handler = handler.clone();
        let source_file = source_file.clone();
        let workspace_root = workspace_root.to_path_buf();
        async move {
            if let Ok(linter_result) =
                linters::typos::run_typos(&source_file, &workspace_root).await
            {
                let _ = handler.handle_linter_result(linter_result);
            }
        }
    }));

    let workspace_root = workspace_root.to_path_buf();

    match filetype {
        "go" => {
            let job = async move { linters::go::run_gopls(&source_file, &workspace_root).await };

            handles.push(spawn_linter_job(job, handler));
        }
        "rust" => {
            handles
                .extend(linters::rust::RustLinter::new(source_file, workspace_root).run(handler));
        }
        "sh" => {
            let job =
                async move { linters::sh::run_shellcheck(&source_file, &workspace_root).await };

            handles.push(spawn_linter_job(job, handler));
        }
        "vim" => {
            let job = async move { linters::vim::run_vint(&source_file, &workspace_root).await };

            handles.push(spawn_linter_job(job, handler));
        }
        _ => {}
    }

    handles
}

fn spawn_linter_job<Handler>(
    job: impl Future<Output = std::io::Result<LinterResult>> + Send + 'static,
    handler: Handler,
) -> tokio::task::JoinHandle<()>
where
    Handler: HandleLinterResult + Send + Sync + Clone + 'static,
{
    tokio::spawn(async move {
        let linter_result = match job.await {
            Ok(res) => res,
            Err(err) => {
                tracing::error!(?err, "Error occurred running linter");
                return;
            }
        };

        if let Err(err) = handler.handle_linter_result(linter_result) {
            tracing::error!(?err, "Error occurred in handling the linter result");
        }
    })
}
