use crate::{Code, Diagnostic, Severity};
use serde::Deserialize;
use std::path::Path;

// [{"file_path": "autoload/clap.vim", "line_number": 212, "column_number": 98, "severity": "warning", "description": "Prefer single quoted strings", "policy_name": "ProhibitUnnecessaryDoubleQuote", "reference": "Google VimScript Style Guide (Strings)"}]
#[derive(Debug, Deserialize)]
struct VintMessage {
    // file_path: String,
    line_number: usize,
    column_number: usize,
    severity: String,
    description: String,
    // policy_name: String,
}

impl VintMessage {
    fn into_diagnostic(self) -> Diagnostic {
        // https://github.com/Vimjas/vint/blob/e12091830f0ae7311066b9d1417951182fb32eb5/vint/linting/config/config_cmdargs_source.py#L94
        let severity = match self.severity.as_str() {
            "error" => Severity::Error,
            "warning" => Severity::Warning,
            "style_problem" => Severity::Style,
            _ => Severity::Unknown,
        };

        Diagnostic {
            line_start: self.line_number,
            line_end: self.line_number,
            column_start: self.column_number,
            column_end: self.column_number + 1,
            code: Code::default(),
            severity,
            message: self.description,
        }
    }
}

pub fn run_vint(source_file: &Path, workspace: &Path) -> std::io::Result<Vec<Diagnostic>> {
    let output = std::process::Command::new("vint")
        .arg("-j")
        .arg(source_file)
        .current_dir(workspace)
        .output()?;

    Ok(output
        .stdout
        .split(|&b| b == b'\n')
        .map(|line| line.strip_suffix(b"\r").unwrap_or(line))
        .filter_map(|line| serde_json::from_slice::<Vec<VintMessage>>(line).ok())
        .flatten()
        .map(|vint_message| vint_message.into_diagnostic())
        .collect())
}
