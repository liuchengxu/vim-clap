use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::sync::OnceLock;

type LanguageId = &'static str;

pub fn language_id_from_path(path: impl AsRef<Path>) -> Option<LanguageId> {
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

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct LanguageConfig {
    /// c-sharp, rust, tsx
    pub name: String,

    /// List of `&filetype` corresponding to this language.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub filetype: Vec<String>,

    /// these indicate project roots <.git, Cargo.toml>
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub root_markers: Vec<String>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub language_servers: Vec<String>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
struct Configuration {
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub language: Vec<LanguageConfig>,

    #[serde(default)]
    pub language_server: HashMap<String, maple_lsp::LanguageServerConfig>,
}

#[derive(Debug)]
struct ConfigurationInner {
    pub languages: HashMap<String, LanguageConfig>,
    pub language_servers: HashMap<String, maple_lsp::LanguageServerConfig>,
}

fn config_inner() -> &'static ConfigurationInner {
    static CELL: OnceLock<ConfigurationInner> = OnceLock::new();

    CELL.get_or_init(|| {
        let languages_toml = include_str!("../../../../../../languages.toml");

        let config: Configuration = toml::from_str(languages_toml)
            .map_err(|err| {
                tracing::error!(?err, "error in languages.toml");
                err
            })
            .unwrap_or_default();

        let Configuration {
            language,
            language_server,
        } = config;

        ConfigurationInner {
            languages: language.into_iter().map(|c| (c.name.clone(), c)).collect(),
            language_servers: language_server,
        }
    })
}

pub fn get_root_markers(language_name: LanguageId) -> Vec<String> {
    let config = config_inner();

    config
        .languages
        .get(language_name)
        .map(|c| c.root_markers.clone())
        .unwrap_or_default()
}

pub fn get_language_server_config(
    language_name: LanguageId,
) -> Option<maple_lsp::LanguageServerConfig> {
    let config = config_inner();

    let language_config = config.languages.get(language_name)?;

    // TODO: Support multiple servers?
    let language_server = language_config.language_servers.first()?;

    let mut language_server_config = config.language_servers.get(language_server).cloned()?;

    // Update the language server config specified in config.toml.
    if let Some(user_config) = maple_config::config()
        .plugin
        .lsp
        .language_server
        .get(language_server.as_str())
    {
        let user_config: serde_json::Value = serde_json::from_str(&user_config.to_string()).ok()?;
        language_server_config.update_config(user_config);
    }

    Some(language_server_config)
}
