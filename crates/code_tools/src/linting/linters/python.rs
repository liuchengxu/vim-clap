use crate::linting::{Code, Diagnostic, DiagnosticSpan, Linter, Severity};
use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Deserialize)]
struct Location {
    column: usize,
    row: usize,
}

// https://github.com/astral-sh/ruff/blob/b3a6f0ce81bfd547d8a01bfe5dee61cb1b8e73b3/crates/ruff_linter/src/message/json.rs#L80
//
// {"cell":null,"code":"E701","end_location":{"column":50,"row":36},"filename":"/Users/xuliucheng/.vim/plugged/vim-clap/pythonx/clap/fzy.py","fix":null,"location":{"column":49,"row":36},"message":"Multiple statements on one line (colon)","noqa_row":36,"url":"https://docs.astral.sh/ruff/rules/multiple-statements-on-one-line-colon"}
#[derive(Debug, Deserialize)]
struct RuffJsonMessage {
    code: String,
    end_location: Location,
    // filename: String,
    // fix: Option<Fix>,
    location: Location,
    message: String,
    // url: String,
}

impl RuffJsonMessage {
    fn into_diagnostic(self) -> Diagnostic {
        let severity = if self.code.starts_with('E') {
            Severity::Error
        } else if self.code.starts_with('W') {
            Severity::Warning
        } else {
            Severity::Unknown
        };

        Diagnostic {
            spans: vec![DiagnosticSpan {
                line_start: self.location.row,
                line_end: self.end_location.row,
                column_start: self.location.column,
                column_end: self.end_location.column,
            }],
            code: Code { code: self.code },
            severity,
            message: self.message,
        }
    }
}

pub struct Ruff;

impl Linter for Ruff {
    const EXE: &'static str = "ruff";

    fn add_args(cmd: &mut tokio::process::Command, source_file: &Path) {
        cmd.arg("check")
            .arg("--output-format=json-lines")
            .arg(source_file);
    }

    fn parse_line(&self, line: &[u8]) -> Option<Diagnostic> {
        serde_json::from_slice::<RuffJsonMessage>(line)
            .map(RuffJsonMessage::into_diagnostic)
            .ok()
    }
}
