use crate::{Code, Diagnostic, Severity};
use once_cell::sync::Lazy;
use regex::Regex;
use std::path::Path;

// /home/xlc/Data0/src/github.com/ethereum-optimism/optimism/op-node/rollup/superchain.go:38:27-43: undefined: eth.XXXXSystemConfig
static RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?m)^([^:]+):([0-9]+):([0-9]+)-([0-9]+): (.+)$").unwrap());

pub fn start_gopls(source_file: &Path, workspace: &Path) -> std::io::Result<Vec<Diagnostic>> {
    // Use relative path as the workspace is specified explicitly, otherwise it's
    // possible to run into a glitch when the directory is a symlink?
    let source_file = source_file.strip_prefix(workspace).unwrap_or(source_file);
    let output = std::process::Command::new("gopls")
        .arg("check")
        .arg(source_file)
        .current_dir(workspace)
        .output()?;

    let stdout = String::from_utf8_lossy(&output.stdout);

    let mut diagnostics = Vec::new();

    for line in stdout.split('\n') {
        if !line.is_empty() {
            for (_, [path, line, column_start, column_end, message]) in
                RE.captures_iter(line).map(|c| c.extract())
            {
                let line = line.parse::<usize>().unwrap();
                let column_start = column_start.parse::<usize>().unwrap();
                let column_end = column_end.parse::<usize>().unwrap();
                diagnostics.push(Diagnostic {
                    line_start: line,
                    line_end: line,
                    column_start,
                    column_end,
                    code: Code::default(),
                    severity: Severity::Error,
                    message: message.to_string(),
                });
            }
        }
    }

    Ok(diagnostics)
}
