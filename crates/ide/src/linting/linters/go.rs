use crate::linting::{Code, Diagnostic, DiagnosticSpan, LintEngine, LinterDiagnostics, Severity};
use once_cell::sync::Lazy;
use regex::Regex;
use std::path::Path;

// /home/xlc/Data0/src/github.com/ethereum-optimism/optimism/op-node/rollup/superchain.go:38:27-43: undefined: eth.XXXXSystemConfig
static RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?m)^([^:]+):([0-9]+):([0-9]+)-([0-9]+): (.+)$")
        .expect("Regex for parsing gopls output must be correct otherwise the upstream format must have been changed")
});

pub async fn run_gopls(
    source_file: &Path,
    workspace_root: &Path,
) -> std::io::Result<LinterDiagnostics> {
    // Use relative path as the workspace is specified explicitly, otherwise it's
    // possible to run into a glitch when the directory is a symlink?
    let source_file = source_file
        .strip_prefix(workspace_root)
        .unwrap_or(source_file);
    let output = tokio::process::Command::new("gopls")
        .arg("check")
        .arg(source_file)
        .current_dir(workspace_root)
        .output()
        .await?;

    let stdout = String::from_utf8_lossy(&output.stdout);

    let mut diagnostics = Vec::new();

    for line in stdout.split('\n') {
        if !line.is_empty() {
            for caps in RE.captures_iter(line) {
                // [path, line, column_start, column_end, message]
                let (Some(line), Some(column_start), Some(column_end), Some(message)) = (
                    caps.get(2).and_then(|m| m.as_str().parse::<usize>().ok()),
                    caps.get(3).and_then(|m| m.as_str().parse::<usize>().ok()),
                    caps.get(4).and_then(|m| m.as_str().parse::<usize>().ok()),
                    caps.get(5).map(|m| m.as_str().to_string()),
                ) else {
                    continue;
                };
                diagnostics.push(Diagnostic {
                    spans: vec![DiagnosticSpan {
                        line_start: line,
                        line_end: line,
                        column_start,
                        column_end,
                    }],
                    code: Code::default(),
                    severity: Severity::Error,
                    message,
                });
            }
        }
    }

    Ok(LinterDiagnostics {
        engine: LintEngine::Gopls,
        diagnostics,
    })
}
