use crate::linting::{Code, Diagnostic, DiagnosticSpan, Linter, Severity};
use once_cell::sync::Lazy;
use regex::Regex;
use std::path::Path;

// /home/xlc/Data0/src/github.com/ethereum-optimism/optimism/op-node/rollup/superchain.go:38:27-43: undefined: eth.XXXXSystemConfig
static RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?m)^([^:]+):([0-9]+):([0-9]+)-([0-9]+): (.+)$")
        .expect("Regex for parsing gopls output must be correct otherwise the upstream format must have been changed")
});

fn parse_line_gopls(line: &[u8]) -> Option<Diagnostic> {
    let line = String::from_utf8_lossy(line);

    if let Some(caps) = RE.captures_iter(&line).next() {
        // [path, line, column_start, column_end, message]
        let (Some(line), Some(column_start), Some(column_end), Some(message)) = (
            caps.get(2).and_then(|m| m.as_str().parse::<usize>().ok()),
            caps.get(3).and_then(|m| m.as_str().parse::<usize>().ok()),
            caps.get(4).and_then(|m| m.as_str().parse::<usize>().ok()),
            caps.get(5).map(|m| m.as_str().to_string()),
        ) else {
            return None;
        };

        return Some(Diagnostic {
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

    None
}

pub struct Gopls;

impl Linter for Gopls {
    const EXE: &'static str = "gopls";

    fn add_args(cmd: &mut tokio::process::Command, source_file: &Path) {
        cmd.arg("check").arg(source_file);
    }

    fn parse_line(&self, line: &[u8]) -> Option<Diagnostic> {
        parse_line_gopls(line)
    }
}
