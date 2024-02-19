use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::OnceLock;

type LanguageId = &'static str;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct LanguageConfiguration {
    /// c-sharp, rust, tsx
    pub name: String,

    /// see the table under https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/#textDocumentItem
    /// csharp, rust, typescriptreact, for the language-server
    #[serde(rename = "language-id")]
    pub language_server_language_id: Option<String>,

    /// these indicate project roots <.git, Cargo.toml>
    #[serde(default)]
    pub root_markers: Vec<String>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Configuration {
    #[serde(default)]
    pub language_server: HashMap<String, maple_lsp::LanguageServerConfig>,
}

// TODO: support more languages.
pub fn get_language_server_config(
    language_id: LanguageId,
) -> Option<maple_lsp::LanguageServerConfig> {
    static CELL: OnceLock<Configuration> = OnceLock::new();

    let config = CELL.get_or_init(|| {
        let languages_toml = include_str!("../../../../../../languages.toml");

        let languages_config: Configuration = toml::from_str(languages_toml)
            .map_err(|err| {
                tracing::error!(?err, "error in languages.toml");
                err
            })
            .unwrap_or_default();

        languages_config
    });

    let language_server = match language_id {
        "rust" => "rust-analyzer",
        "go" => "gopls",
        _ => return None,
    };

    config.language_server.get(language_server).cloned()
}
