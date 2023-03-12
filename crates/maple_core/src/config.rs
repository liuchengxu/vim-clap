use crate::dirs::PROJECT_DIRS;
use crate::paths::AbsPathBuf;
use once_cell::sync::OnceCell;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

static CONFIG_FILE: OnceCell<PathBuf> = OnceCell::new();

// Should be only initialized once.
pub fn initialize_config_file(specified_file: Option<PathBuf>) {
    let config_file = specified_file.unwrap_or_else(|| {
        // Linux: ~/.config/vimclap/config.toml
        // macOS: ~/Library/Application\ Support/org.vim.Vim-Clap/config.toml
        let config_file_path = PROJECT_DIRS.config_dir().join("config.toml");

        if !config_file_path.exists() {
            std::fs::create_dir_all(&config_file_path).ok();
        }

        config_file_path
    });

    CONFIG_FILE.set(config_file).ok();
}

pub fn config_file() -> &'static PathBuf {
    CONFIG_FILE.get().expect("Config file uninitialized")
}

// TODO: reload-config
pub fn config() -> &'static Config {
    static CONFIG: OnceCell<Config> = OnceCell::new();

    CONFIG.get_or_init(|| {
        std::fs::read_to_string(CONFIG_FILE.get().expect("Config file uninitialized!"))
            .and_then(|contents| {
                toml::from_str(&contents).map_err(|err| {
                    // TODO: Notify the config error.
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

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "kebab-case", default, deny_unknown_fields)]
pub struct MatcherConfig {
    pub tiebreak: String,
}

impl Default for MatcherConfig {
    fn default() -> Self {
        Self {
            tiebreak: "score,-begin,-end,-length".into(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "kebab-case", default, deny_unknown_fields)]
pub struct LogConfig {
    pub log_file: Option<String>,
    pub max_level: String,
}

impl Default for LogConfig {
    fn default() -> Self {
        Self {
            log_file: None,
            max_level: "debug".into(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "kebab-case", default, deny_unknown_fields)]
pub struct HighlightCursorWordConfig {
    /// Whether to enable this plugin.
    pub enable: bool,
    /// Whether to ignore the comment line
    pub ignore_comment_line: bool,
    /// Disable the plugin when the file matches this pattern.
    pub ignore_files: String,
}

impl Default for HighlightCursorWordConfig {
    fn default() -> Self {
        Self {
            enable: false,
            ignore_comment_line: false,
            ignore_files: "*.toml,*.json,*.yml,*.log,tmp".to_string(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(rename_all = "kebab-case", default, deny_unknown_fields)]
pub struct PluginConfig {
    pub highlight_cursor_word: HighlightCursorWordConfig,
}

#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(rename_all = "kebab-case", default, deny_unknown_fields)]
pub struct Config {
    /// Log configuration.
    pub log: LogConfig,

    /// Matcher configuration.
    pub matcher: MatcherConfig,

    /// Plugin configuration.
    pub plugin: PluginConfig,

    /// Global ignore configuration.
    pub global_ignore: IgnoreConfig,

    /// Ignore configuration per project.
    ///
    /// The project path must be specified as absolute path or a path relative to the home directory.
    pub project_ignore: HashMap<AbsPathBuf, IgnoreConfig>,

    /// Ignore configuration per provider.
    ///
    /// Priorities of the ignore config:
    ///   provider_ignores > provider_ignores > global_ignore
    pub provider_ignore: HashMap<String, IgnoreConfig>,
}

impl Config {
    pub fn ignore_config(&self, provider_id: &str, project_dir: &AbsPathBuf) -> &IgnoreConfig {
        self.provider_ignore.get(provider_id).unwrap_or_else(|| {
            self.project_ignore
                .get(project_dir)
                .unwrap_or(&self.global_ignore)
        })
    }
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
          [global-ignore]
          file-path-pattern = ["test", "build"]

          # [project-ignore."~/src/github.com/subspace/subspace"]
          # comment-line = true

          [provider-ignore.dumb_jump]
          comment-line = true

          [log]
          max-level = "trace"
          log-file = "/tmp/clap.log"

          [matcher]
          tiebreak = "score,-begin,-end,-length"
"#;
        let user_config: Config = toml::from_str(toml_content).unwrap();
        println!("{user_config:?}");
        println!("{}", toml::to_string(&user_config).unwrap());
    }
}
