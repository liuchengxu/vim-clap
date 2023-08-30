mod rust;

use lsp_types::DiagnosticSeverity;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug)]
pub enum Linter {
    Rust,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Code {
    pub code: String,
    pub explanation: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Diagnostic {
    pub line_start: usize,
    pub line_end: usize,
    pub column_start: usize,
    pub column_end: usize,
    pub code: Code,
    pub severity: Option<DiagnosticSeverity>,
    pub message: String,
}

impl Diagnostic {
    pub fn human_message(&self) -> String {
        format!("[{}] {}", self.code.code, self.message)
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct PartialSpan {
    line_start: usize,
    line_end: usize,
    column_start: usize,
    column_end: usize,
    file_name: String,
    label: Option<String>,
    level: Option<String>,
    rendered: Option<String>,
}

pub fn lint(source_file: impl AsRef<Path>, workspace: &Path) -> std::io::Result<Vec<Diagnostic>> {
    let linter = self::rust::RustLinter {
        source_file,
        workspace,
    };

    let mut diagonostics = linter.cargo_check()?;
    // diagonostics.extend(linter.cargo_clippy()?);
    Ok(diagonostics)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cargo() {
        let source_file =
            Path::new("/Users/xuliucheng/.vim/plugged/vim-clap/crates/linter/src/lib.rs");

        let workspace = paths::find_project_root(&source_file, &["Cargo.toml"]).unwrap();
        let diagonostics = lint(&source_file, workspace);
        println!("======= diagonostics: {diagonostics:#?}");
    }
}
