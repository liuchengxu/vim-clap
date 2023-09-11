use crate::{Code, Diagnostic, Severity};
use std::borrow::Cow;
use std::path::Path;

#[derive(Clone, PartialEq, Eq, Debug, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
#[serde(untagged)]
enum Status<'c> {
    Valid,
    Invalid,
    Corrections(Vec<Cow<'c, str>>),
}

impl<'c> Status<'c> {
    fn message(&self) -> Cow<'c, str> {
        match self {
            Self::Valid => "valid".into(),
            Self::Invalid => "invalid".into(),
            Self::Corrections(corrections) => {
                format!("corrections: {}", corrections.join(",")).into()
            }
        }
    }
}

#[derive(Clone, Debug, serde::Deserialize)]
#[non_exhaustive]
struct FileContext {
    #[allow(unused)]
    pub path: String,
    pub line_num: usize,
}

#[derive(Clone, Debug, serde::Deserialize)]
#[non_exhaustive]
struct PathContext {
    #[allow(unused)]
    pub path: String,
}

#[derive(Clone, Debug, serde::Deserialize)]
#[serde(untagged)]
#[non_exhaustive]
enum Context {
    File(FileContext),
    Path(PathContext),
}

impl Context {
    fn line_num(&self) -> Option<usize> {
        if let Context::File(file_context) = self {
            Some(file_context.line_num)
        } else {
            None
        }
    }
}

#[derive(Clone, Debug, serde::Deserialize)]
#[non_exhaustive]
struct Typo<'c> {
    #[serde(flatten)]
    context: Option<Context>,
    byte_offset: usize,
    typo: String,
    corrections: Status<'c>,
}

// https://github.com/crate-ci/typos/blob/65d2fb6b91a696bfff5d59e7cf960cd7e51fb83a/crates/typos-cli/src/report.rs#L13C23-L13C23
#[derive(Clone, Debug, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
#[serde(tag = "type")]
#[non_exhaustive]
enum Message<'m> {
    Typo(Typo<'m>),
}

impl<'m> Message<'m> {
    fn try_into_diagnostic(self) -> Option<Diagnostic> {
        match self {
            Self::Typo(typo) => {
                let Typo {
                    context,
                    byte_offset,
                    typo,
                    corrections,
                } = typo;

                if let Some(line_num) = context.and_then(|cx| cx.line_num()) {
                    let message = corrections.message().into_owned();
                    Some(Diagnostic {
                        line_start: line_num,
                        line_end: line_num,
                        column_start: byte_offset + 1,
                        column_end: byte_offset + 1 + typo.len(),
                        code: Code::default(),
                        severity: Severity::Warning,
                        message,
                    })
                } else {
                    None
                }
            }
        }
    }
}

pub fn run_typos(source_file: &Path, workspace: &Path) -> std::io::Result<Vec<Diagnostic>> {
    let output = std::process::Command::new("typos")
        .arg("--format=json")
        .arg(source_file)
        .current_dir(workspace)
        .output()?;

    Ok(output
        .stdout
        .split(|&b| b == b'\n')
        .map(|line| line.strip_suffix(b"\r").unwrap_or(line))
        .filter_map(|line| {
            serde_json::from_slice::<Message>(line)
                .ok()
                .and_then(|message| message.try_into_diagnostic())
        })
        .collect())
}
