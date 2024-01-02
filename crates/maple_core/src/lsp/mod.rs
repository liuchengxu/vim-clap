use crate::stdio_server::Vim;
use maple_lsp::{
    lsp, HandleLanguageServerMessage, LanguageServerNotification, LanguageServerRequest,
};
use serde_json::Value;
use std::path::Path;
use std::time::Instant;

pub fn language_id_from_path(path: impl AsRef<Path>) -> Option<&'static str> {
    // recommended language_id values
    // https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/#textDocumentItem
    Some(match path.as_ref().extension() {
        Some(ext) => {
            match ext.to_str()? {
                "C" | "H" => "cpp",
                "M" => "objective-c",
                // stop case-sensitive matching
                ext => match ext.to_lowercase().as_str() {
                    "bat" => "bat",
                    "clj" | "cljs" | "cljc" | "edn" => "clojure",
                    "coffee" => "coffeescript",
                    "c" | "h" => "c",
                    "cpp" | "hpp" | "cxx" | "hxx" | "c++" | "h++" | "cc" | "hh" => "cpp",
                    "cs" | "csx" => "csharp",
                    "css" => "css",
                    "d" | "di" | "dlang" => "dlang",
                    "diff" | "patch" => "diff",
                    "dart" => "dart",
                    "dockerfile" => "dockerfile",
                    "elm" => "elm",
                    "ex" | "exs" => "elixir",
                    "erl" | "hrl" => "erlang",
                    "fs" | "fsi" | "fsx" | "fsscript" => "fsharp",
                    "git-commit" | "git-rebase" => "git",
                    "go" => "go",
                    "groovy" | "gvy" | "gy" | "gsh" => "groovy",
                    "hbs" => "handlebars",
                    "htm" | "html" | "xhtml" => "html",
                    "ini" => "ini",
                    "java" | "class" => "java",
                    "js" => "javascript",
                    "jsx" => "javascriptreact",
                    "json" => "json",
                    "jl" => "julia",
                    "kt" | "kts" => "kotlin",
                    "less" => "less",
                    "lua" => "lua",
                    "makefile" | "gnumakefile" => "makefile",
                    "md" | "markdown" => "markdown",
                    "m" => "objective-c",
                    "mm" => "objective-cpp",
                    "plx" | "pl" | "pm" | "xs" | "t" | "pod" | "cgi" => "perl",
                    "p6" | "pm6" | "pod6" | "t6" | "raku" | "rakumod" | "rakudoc" | "rakutest" => {
                        "perl6"
                    }
                    "php" | "phtml" | "pht" | "phps" => "php",
                    "proto" => "proto",
                    "ps1" | "ps1xml" | "psc1" | "psm1" | "psd1" | "pssc" | "psrc" => "powershell",
                    "py" | "pyi" | "pyc" | "pyd" | "pyw" => "python",
                    "r" => "r",
                    "rb" => "ruby",
                    "rs" => "rust",
                    "scss" | "sass" => "scss",
                    "sc" | "scala" => "scala",
                    "sh" | "bash" | "zsh" => "shellscript",
                    "sql" => "sql",
                    "swift" => "swift",
                    "svelte" => "svelte",
                    "thrift" => "thrift",
                    "toml" => "toml",
                    "ts" => "typescript",
                    "tsx" => "typescriptreact",
                    "tex" => "tex",
                    "vb" => "vb",
                    "xml" | "csproj" => "xml",
                    "xsl" => "xsl",
                    "yml" | "yaml" => "yaml",
                    "zig" => "zig",
                    "vue" => "vue",
                    _ => return None,
                },
            }
        }
        None => {
            // Handle paths without extension
            let filename = path.as_ref().file_name()?.to_str()?;

            let language_id = match filename.to_lowercase().as_str() {
                "dockerfile" => "dockerfile",
                "makefile" | "gnumakefile" => "makefile",
                _ => return None,
            };

            language_id
        }
    })
}

pub fn find_lsp_root<'a>(language_id: &str, path: &'a Path) -> Option<&'a Path> {
    let find = |root_markers| paths::find_project_root(path, root_markers);
    match language_id {
        "c" | "cpp" => find(&["compile_commands.json"]),
        "java" => find(&["pom.xml", "settings.gradle", "settings.gradle.kts"]),
        "javascript" | "typescript" | "javascript.jsx" | "typescript.tsx" => {
            find(&["package.json"])
        }
        "php" => find(&["composer.json"]),
        "python" => find(&["setup.py", "Pipfile", "requirements.txt", "pyproject.toml"]),
        "rust" => find(&["Cargo.toml"]),
        "scala" => find(&["build.sbt"]),
        "haskell" => find(&["stack.yaml"]),
        "go" => find(&["go.mod"]),
        _ => paths::find_project_root(path, &[".git", ".hg", ".svn"]).or_else(|| path.parent()),
    }
}

#[derive(Debug)]
pub struct LanguageServerMessageHandler {
    server_name: String,
    last_lsp_update: Option<Instant>,
    vim: Vim,
}

impl LanguageServerMessageHandler {
    const LSP_UPDATE_DELAY: u128 = 50;

    pub fn new(server_name: String, vim: Vim) -> Self {
        Self {
            server_name,
            vim,
            last_lsp_update: None,
        }
    }

    /// Update the lsp status if a certain time delay has passed since the last update.
    fn update_lsp_status_gentlely(&mut self, new: Option<String>) {
        let should_update = match self.last_lsp_update {
            Some(last_update) => last_update.elapsed().as_millis() > Self::LSP_UPDATE_DELAY,
            None => true,
        };

        if should_update {
            let _ = self
                .vim
                .update_lsp_status(new.as_ref().unwrap_or(&self.server_name));
            self.last_lsp_update.replace(Instant::now());
        }
    }

    fn handle_progress_message(
        &mut self,
        params: lsp::ProgressParams,
    ) -> Result<(), maple_lsp::Error> {
        use lsp::{
            NumberOrString, ProgressParams, ProgressParamsValue, WorkDoneProgress,
            WorkDoneProgressBegin, WorkDoneProgressEnd, WorkDoneProgressReport,
        };

        let ProgressParams { token, value } = params;

        let ProgressParamsValue::WorkDone(work) = value;

        let parts = match &work {
            WorkDoneProgress::Begin(WorkDoneProgressBegin {
                title,
                message,
                percentage,
                ..
            }) => (Some(title), message, percentage),
            WorkDoneProgress::Report(WorkDoneProgressReport {
                message,
                percentage,
                ..
            }) => (None, message, percentage),
            WorkDoneProgress::End(WorkDoneProgressEnd { message }) => {
                if message.is_some() {
                    (None, message, &None)
                } else {
                    // End progress.
                    let _ = self.vim.update_lsp_status(&self.server_name);

                    // we want to render to clear any leftover spinners or messages
                    return Ok(());
                }
            }
        };

        let token_d: &dyn std::fmt::Display = match &token {
            NumberOrString::Number(n) => n,
            NumberOrString::String(s) => s,
        };

        let status = match parts {
            (Some(title), Some(message), Some(percentage)) => {
                format!("[{}] {}% {} - {}", token_d, percentage, title, message)
            }
            (Some(title), None, Some(percentage)) => {
                format!("[{}] {}% {}", token_d, percentage, title)
            }
            (Some(title), Some(message), None) => {
                format!("[{}] {} - {}", token_d, title, message)
            }
            (None, Some(message), Some(percentage)) => {
                format!("[{}] {}% {}", token_d, percentage, message)
            }
            (Some(title), None, None) => {
                format!("[{}] {}", token_d, title)
            }
            (None, Some(message), None) => {
                format!("[{}] {}", token_d, message)
            }
            (None, None, Some(percentage)) => {
                format!("[{}] {}%", token_d, percentage)
            }
            (None, None, None) => format!("[{}]", token_d),
        };

        if let WorkDoneProgress::End(_) = work {
            let _ = self.vim.update_lsp_status(&self.server_name);
        } else {
            self.update_lsp_status_gentlely(Some(status));
        }

        Ok(())
    }

    fn handle_publish_diagnostics(
        &mut self,
        params: lsp::PublishDiagnosticsParams,
    ) -> Result<(), maple_lsp::Error> {
        let _filename = params.uri.path();
        Ok(())
    }
}

impl HandleLanguageServerMessage for LanguageServerMessageHandler {
    fn handle_request(
        &mut self,
        id: rpc::Id,
        request: LanguageServerRequest,
    ) -> Result<Value, rpc::Error> {
        tracing::debug!(%id, "Processing language server request: {request:?}");

        // match request {
        // LanguageServerRequest::WorkDoneProgressCreate(_params) => {}
        // _ => {}
        // }

        Ok(Value::Null)
    }

    fn handle_notification(
        &mut self,
        notification: LanguageServerNotification,
    ) -> Result<(), maple_lsp::Error> {
        tracing::debug!("Processing language server notification: {notification:?}");

        match notification {
            LanguageServerNotification::ProgressMessage(params) => {
                self.handle_progress_message(params)?;
            }
            LanguageServerNotification::PublishDiagnostics(params) => {
                self.handle_publish_diagnostics(params)?;
            }
            _ => {
                tracing::debug!("TODO: handle language server notification");
            }
        }

        Ok(())
    }
}
