type LanguageId = &'static str;

#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct LanguageConfig {
    /// c-sharp, rust, tsx
    #[serde(rename = "name")]
    pub language_id: String,

    /// see the table under https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/#textDocumentItem
    /// csharp, rust, typescriptreact, for the language-server
    #[serde(rename = "language-id")]
    pub language_server_language_id: Option<String>,

    /// these indicate project roots <.git, Cargo.toml>
    #[serde(default)]
    pub root_markers: Vec<String>,
}

// TODO: support more languages.
pub fn get_language_config(language_id: LanguageId) -> Option<maple_lsp::LanguageConfig> {
    let language_config = match language_id {
        "rust" => maple_lsp::LanguageConfig {
            cmd: String::from("rust-analyzer"),
            args: vec![],
            root_markers: vec![String::from("Cargo.toml")],
        },
        "go" => maple_lsp::LanguageConfig {
            cmd: String::from("gopls"),
            args: vec![],
            root_markers: vec![String::from("go.mod")],
        },
        _ => return None,
    };

    Some(language_config)
}
