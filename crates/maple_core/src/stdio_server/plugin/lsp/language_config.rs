use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::OnceLock;

type LanguageId = &'static str;

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

pub fn get_root_markers<'a>(language_name: LanguageId) -> Vec<String> {
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

    // TODO: Only support the first server for now.
    let language_server = language_config.language_servers.first()?;

    config.language_servers.get(language_server).cloned()
}
