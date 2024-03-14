mod linters;

use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::sync::mpsc::UnboundedSender;

#[derive(Serialize, Deserialize, Default, Debug, Clone, Eq, PartialEq)]
pub struct Code {
    pub code: String,
    // Ignore `explanation` as it is too verbose and nevery displayed.
    // pub explanation: Option<String>,
}

// Diagnostic severity.
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

impl DiagnosticSpan {
    pub fn start_pos(&self) -> (usize, usize) {
        (self.line_start, self.column_start)
    }
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

        // If the diagnostics point to the same lines and have the same display message, they are considered
        // as the same as they are visually indistinguishable.
        if self.spans.iter().zip(other.spans.iter()).all(|(a, b)| {
            a.line_start == a.line_end && a.line_start == b.line_start && b.line_start == b.line_end
        }) && is_same_code()
            && self.message == other.message
        {
            return true;
        }

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

    pub fn is_error(&self) -> bool {
        matches!(self.severity, Severity::Error)
    }

    pub fn is_warn(&self) -> bool {
        matches!(self.severity, Severity::Warning)
    }
}

#[derive(Debug, Clone)]
pub struct LinterDiagnostics {
    pub source: &'static str,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Debug, Clone)]
enum WorkspaceMarker {
    RootMarkers(&'static [&'static str]),
    /// Use the parent directory as the workspace_root if no explicit root markers.
    ParentOfSourceFile,
}

impl WorkspaceMarker {
    fn find_workspace<'a>(&'a self, source_file: &'a Path) -> Option<&Path> {
        match self {
            Self::RootMarkers(root_markers) => paths::find_project_root(source_file, root_markers),
            Self::ParentOfSourceFile => Some(source_file.parent().unwrap_or(source_file)),
        }
    }
}

/// Returns the working directory for running the command of lint engine.
pub fn find_workspace(filetype: impl AsRef<str>, source_file: &Path) -> Option<&Path> {
    use WorkspaceMarker::{ParentOfSourceFile, RootMarkers};

    static WORKSPACE_MARKERS: Lazy<HashMap<&str, WorkspaceMarker>> = Lazy::new(|| {
        HashMap::from([
            ("go", RootMarkers(&["go.mod", ".git"])),
            ("rust", RootMarkers(&["Cargo.toml"])),
            ("sh", ParentOfSourceFile),
            ("vim", ParentOfSourceFile),
            ("markdown", ParentOfSourceFile),
            ("python", ParentOfSourceFile),
        ])
    });

    WORKSPACE_MARKERS
        .get(filetype.as_ref())
        .and_then(|workspace_marker| workspace_marker.find_workspace(source_file))
}

trait Linter {
    const EXE: &'static str;

    /// Constructs a base linter command.
    fn base_command(workspace_root: &Path) -> std::io::Result<tokio::process::Command> {
        let executable = which::which(Self::EXE)
            .map_err(|err| std::io::Error::new(std::io::ErrorKind::Other, err.to_string()))?;

        let mut cmd = tokio::process::Command::new(executable);

        cmd.current_dir(workspace_root);

        Ok(cmd)
    }

    /// Append the linter-specific args to the linter executable.
    fn add_args(base_cmd: &mut tokio::process::Command, source_file: &Path);

    /// Constructs the final linter command.
    fn command(
        source_file: &Path,
        workspace_root: &Path,
    ) -> std::io::Result<tokio::process::Command> {
        let mut cmd = Self::base_command(workspace_root)?;
        Self::add_args(&mut cmd, source_file);
        Ok(cmd)
    }

    /// Parses the diagnostic message from linter in a line-wise manner.
    ///
    /// At most one diagnostic per line. This method must be implemented if the default
    /// implementation of `Self::lint_file` is used.
    fn parse_line(&self, _line: &[u8]) -> Option<Diagnostic> {
        unimplemented!("line-wise parser unimplemented for linter {}", Self::EXE)
    }

    /// Starts linting a file and returns the diagnostics.
    async fn lint_file(
        &self,
        source_file: &Path,
        workspace_root: &Path,
    ) -> std::io::Result<LinterDiagnostics> {
        let mut cmd = Self::command(source_file, workspace_root)?;

        let output = cmd.output().await?;

        let diagnostics = output
            .stdout
            .split(|&b| b == b'\n')
            .map(|line| line.strip_suffix(b"\r").unwrap_or(line))
            .filter_map(|line| self.parse_line(line))
            .collect();

        Ok(LinterDiagnostics {
            source: Self::EXE,
            diagnostics,
        })
    }
}

async fn start_linting(
    filetype: &str,
    source_file: PathBuf,
    workspace_root: &Path,
    diagnostics_sender: UnboundedSender<LinterDiagnostics>,
) {
    // Use relative path as the workspace root is always specified explicitly,
    // otherwise it's possible to run into a glitch when the directory is a symlink for gopls?
    let source_file = source_file
        .strip_prefix(workspace_root)
        .map(|p| p.to_path_buf())
        .unwrap_or(source_file);

    tokio::spawn({
        let source_file = source_file.clone();
        let workspace_root = workspace_root.to_path_buf();
        let diagnostics_sender = diagnostics_sender.clone();

        async move {
            if let Ok(diagnostics) = linters::typos::Typos
                .lint_file(&source_file, &workspace_root)
                .await
            {
                if !diagnostics.diagnostics.is_empty() {
                    let _ = diagnostics_sender.send(diagnostics);
                }
            }
        }
    });

    let workspace_root = workspace_root.to_path_buf();

    let diagnostics_result = match filetype {
        "go" => {
            linters::go::Gopls
                .lint_file(&source_file, &workspace_root)
                .await
        }
        "sh" => {
            linters::sh::ShellCheck
                .lint_file(&source_file, &workspace_root)
                .await
        }
        "vim" => {
            linters::vim::Vint
                .lint_file(&source_file, &workspace_root)
                .await
        }
        "python" => {
            linters::python::Ruff
                .lint_file(&source_file, &workspace_root)
                .await
        }
        "rust" => {
            linters::rust::RustLinter::new(source_file, workspace_root).start(diagnostics_sender);
            return;
        }
        _ => {
            return;
        }
    };

    if let Ok(diagnostics) = diagnostics_result {
        if !diagnostics.diagnostics.is_empty() {
            let _ = diagnostics_sender.send(diagnostics);
        }
    }
}

pub fn start_linting_in_background(
    filetype: String,
    source_file: PathBuf,
    workspace_root: PathBuf,
    diagnostics_sender: UnboundedSender<LinterDiagnostics>,
) {
    tokio::spawn(async move {
        start_linting(&filetype, source_file, &workspace_root, diagnostics_sender).await;
    });
}
