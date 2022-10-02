use std::collections::HashMap;
use std::path::PathBuf;

use once_cell::sync::OnceCell;
use serde::{Deserialize, Serialize};

use crate::utils::PROJECT_DIRS;

static CONFIG_FILE: OnceCell<PathBuf> = OnceCell::new();

// Should be only initialized once.
pub fn initialize_config_file(specified_file: Option<PathBuf>) {
    let config_file = specified_file.unwrap_or_else(|| {
        // Linux: ~/.config/vimclap/config.toml
        let config_dir = PROJECT_DIRS.config_dir().to_path_buf();

        if !config_dir.exists() {
            std::fs::create_dir_all(&config_dir).ok();
        }

        config_dir.join("config.toml")
    });

    CONFIG_FILE.set(config_file).ok();
}

// TODO: reload-config
pub fn config() -> &'static Config {
    static CONFIG: OnceCell<Config> = OnceCell::new();

    CONFIG.get_or_init(|| {
        std::fs::read_to_string(CONFIG_FILE.get().expect("Config file uninitialized!"))
            .and_then(|contents| {
                toml::from_str(&contents).map_err(|err| {
                    tracing::debug!(
                        ?err,
                        "Error while deserializing config.toml, using the default config"
                    );
                    std::io::Error::new(
                        std::io::ErrorKind::Other,
                        format!("Error occurred at reading config.toml: {err}"),
                    )
                })
            })
            .unwrap_or_default()
    })
}

#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(rename_all = "kebab-case", default, deny_unknown_fields)]
pub struct Config {
    #[serde(default)]
    pub provider: ProviderConfig,
}

impl Config {
    pub fn provider_ignore_config(&self, provider_id: &str, project_dir: &str) -> &IgnoreConfig {
        self.provider
            .provider_ignore
            .get(provider_id)
            .unwrap_or_else(|| {
                self.provider
                    .provider_ignore
                    .get(project_dir)
                    .unwrap_or(&self.provider.global_ignore)
            })
    }
}

#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(rename_all = "kebab-case", default, deny_unknown_fields)]
pub struct ProviderConfig {
    /// Global ignore configuration.
    pub global_ignore: IgnoreConfig,
    /// Ignore configuration per project.
    ///
    /// Absolute path string is recommended.
    pub project_ignore: HashMap<String, IgnoreConfig>,
    /// Ignore configuration per provider.
    ///
    /// Priorities of the ignore config:
    ///   provider_ignores > provider_ignores > global_ignore
    pub provider_ignore: HashMap<String, IgnoreConfig>,
}

#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(rename_all = "kebab-case", default, deny_unknown_fields)]
pub struct IgnoreConfig {
    /// Whether to ignore the comment line when it's possible.
    pub comment_line: bool,
    /// Only include the results from the files being tracked by git if in a git repo.
    pub git_tracked_only: bool,
    /// Ignore the results from the files whose file name matches this pattern.
    pub file_name_pattern: Vec<String>,
    /// Ignore the results from the files whose file path matches this pattern.
    pub file_path_pattern: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_config() {
        let toml_content = r#"
          [provider.global-ignore]
          file-path-pattern = ["test", "build"]

          [provider.project-ignore."~/src/github.com/subspace/subspace"]
          comment-line = true
"#;
        let user_config: Config = toml::from_str(toml_content).unwrap();
        println!("{user_config:?}");
        println!("{}", toml::to_string(&user_config).unwrap());
    }
}
