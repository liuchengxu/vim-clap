use crate::{Code, Diagnostic, DiagnosticSpan, LinterResult, Severity};
use once_cell::sync::Lazy;
use regex::Regex;
use std::path::Path;

// /home/xlc/Data0/src/github.com/ethereum-optimism/optimism/op-node/rollup/superchain.go:38:27-43: undefined: eth.XXXXSystemConfig
static RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?m)^([^:]+):([0-9]+):([0-9]+)-([0-9]+): (.+)$")
        .expect("Regex for parsing gopls output must be correct otherwise the format is changed")
});

pub async fn run_gopls(source_file: &Path, workspace_root: &Path) -> std::io::Result<LinterResult> {
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

    // for line in stdout.split('\n') {
    // if !line.is_empty() {
    // for (_, [_path, line, column_start, column_end, message]) in
    // RE.captures_iter(line).map(|c| c.extract())
    // {
    // let Ok(line) = line.parse::<usize>() else {
    // continue;
    // };
    // let Ok(column_start) = column_start.parse::<usize>() else {
    // continue;
    // };
    // let Ok(column_end) = column_end.parse::<usize>() else {
    // continue;
    // };
    // diagnostics.push(Diagnostic {
    // spans: vec![DiagnosticSpan {
    // line_start: line,
    // line_end: line,
    // column_start,
    // column_end,
    // }],
    // code: Code::default(),
    // severity: Severity::Error,
    // message: message.to_string(),
    // });
    // }
    // }
    // }

    Ok(LinterResult {
        engine: crate::LintEngine::Gopls,
        diagnostics,
    })
}
