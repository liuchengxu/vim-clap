use dirs::Dirs;
use once_cell::sync::OnceCell;
use paths::AbsPathBuf;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use types::RankCriterion;

static CONFIG_FILE: OnceCell<PathBuf> = OnceCell::new();
// TODO: reload-config
static CONFIG: OnceCell<Config> = OnceCell::new();

pub fn load_config_on_startup(
    specified_config_file: Option<PathBuf>,
) -> (&'static Config, Option<toml::de::Error>) {
    let config_file = specified_config_file.unwrap_or_else(|| {
        // Linux: ~/.config/vimclap/config.toml
        // macOS: ~/Library/Application\ Support/org.vim.Vim-Clap/config.toml
        // Windows: ~\AppData\Roaming\Vim\Vim Clap\config\config.toml
        let config_file_path = Dirs::project().config_dir().join("config.toml");

        if !config_file_path.exists() {
            std::fs::create_dir_all(&config_file_path).ok();
        }

        config_file_path
    });

    let mut maybe_config_err = None;
    let loaded_config = std::fs::read_to_string(&config_file)
        .and_then(|contents| {
            toml::from_str(&contents).map_err(|err| {
                maybe_config_err.replace(err);
                std::io::Error::new(std::io::ErrorKind::Other, "Error occurred in config.toml")
            })
        })
        .unwrap_or_default();

    CONFIG_FILE
        .set(config_file)
        .expect("Failed to initialize Config file");

    CONFIG
        .set(loaded_config)
        .expect("Failed to initialize Config");

    (config(), maybe_config_err)
}

pub fn config() -> &'static Config {
    CONFIG.get().expect("Config must be initialized")
}

pub fn config_file() -> &'static PathBuf {
    CONFIG_FILE.get().expect("Config file uninitialized")
}

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
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

impl MatcherConfig {
    pub fn rank_criteria(&self) -> Vec<RankCriterion> {
        self.tiebreak
            .split(',')
            .filter_map(|s| types::parse_criteria(s.trim()))
            .collect()
    }
}

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
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

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
#[serde(rename_all = "kebab-case", default, deny_unknown_fields)]
pub struct CursorWordConfig {
    /// Whether to enable this plugin.
    pub enable: bool,
    /// Whether to ignore the comment line
    pub ignore_comment_line: bool,
    /// Disable the plugin when the file matches this pattern.
    pub ignore_files: String,
}

impl Default for CursorWordConfig {
    fn default() -> Self {
        Self {
            enable: false,
            ignore_comment_line: false,
            ignore_files: "*.toml,*.json,*.yml,*.log,tmp".to_string(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Default, Eq, PartialEq)]
#[serde(rename_all = "kebab-case", default, deny_unknown_fields)]
pub struct MarkdownPluginConfig {
    /// Whether to enable this plugin.
    pub enable: bool,
}

#[derive(Serialize, Deserialize, Debug, Default, Eq, PartialEq)]
#[serde(rename_all = "kebab-case", default, deny_unknown_fields)]
pub struct CtagsPluginConfig {
    /// Whether to enable this plugin.
    pub enable: bool,
}

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
#[serde(rename_all = "kebab-case", default, deny_unknown_fields)]
pub struct GitPluginConfig {
    /// Whether to enable this plugin.
    pub enable: bool,

    /// Format string used to display the blame info.
    ///
    /// The default format is `"(author time) summary"`.
    pub blame_format_string: Option<String>,
}

impl Default for GitPluginConfig {
    fn default() -> Self {
        Self {
            enable: true,
            blame_format_string: None,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Default, Eq, PartialEq)]
#[serde(rename_all = "kebab-case", default, deny_unknown_fields)]
pub struct ColorizerPluginConfig {
    /// Whether to enable this plugin.
    pub enable: bool,
}

#[derive(Serialize, Deserialize, Debug, Default, Eq, PartialEq)]
#[serde(rename_all = "kebab-case", default, deny_unknown_fields)]
pub struct LinterPluginConfig {
    /// Whether to enable this plugin.
    pub enable: bool,
}

#[derive(Serialize, Deserialize, Debug, Default, Eq, PartialEq)]
#[serde(rename_all = "kebab-case", default, deny_unknown_fields)]
pub struct PluginConfig {
    pub colorizer: ColorizerPluginConfig,
    pub cursorword: CursorWordConfig,
    pub ctags: CtagsPluginConfig,
    pub git: GitPluginConfig,
    pub linter: LinterPluginConfig,
    pub markdown: MarkdownPluginConfig,
}

#[derive(Serialize, Deserialize, Debug, Default, Eq, PartialEq)]
#[serde(rename_all = "kebab-case", default, deny_unknown_fields)]
pub struct IgnoreConfig {
    /// Whether to ignore the comment line when it's possible.
    pub ignore_comments: bool,
    /// Only include the results from the files being tracked by git if in a git repo.
    pub git_tracked_only: bool,
    /// Ignore the results from the files whose file name matches this pattern.
    pub ignore_file_name_pattern: Vec<String>,
    /// Ignore the results from the files whose file path matches this pattern.
    pub ignore_file_path_pattern: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, Default, Eq, PartialEq)]
#[serde(rename_all = "kebab-case", default, deny_unknown_fields)]
pub struct ProviderConfig {
    /// Delay in milliseconds before the user query will be handled actually.
    ///
    /// When enabled and not-zero, some intermediate inputs will be dropped if user types too fast.
    ///
    /// # Config example
    ///
    /// ```toml
    /// [provider.debounce]
    /// # Set debounce to 200ms for all providers by default.
    /// "*" = 200
    ///
    /// # Set debounce to 100ms for files provider specifically.
    /// "files" = 100
    /// ```
    pub debounce: HashMap<String, u64>,

    /// Ignore configuration per provider.
    ///
    /// Priorities of the ignore config:
    ///   provider_ignores > provider_ignores > global_ignore
    pub ignore: HashMap<String, IgnoreConfig>,

    /// Specifies how many items will be displayed in the results window.
    pub max_display_size: Option<usize>,

    /// Specify the syntax highlight engine for the provider preview.
    pub preview_highlight_engine: HighlightEngine,

    /// Specify the theme for the highlight engine.
    ///
    /// If the theme is not found and the engine is [`HighlightEngine::SublimeSyntax`],
    /// the default theme (`Visual Studio Dark+`) will be used.
    pub preview_color_scheme: Option<String>,

    /// Whether to share the input history of each provider.
    pub share_input_history: bool,
}

#[derive(Serialize, Deserialize, Debug, Default, Eq, PartialEq)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub enum HighlightEngine {
    SublimeSyntax,
    TreeSitter,
    #[default]
    Vim,
}

#[derive(Serialize, Deserialize, Debug, Default, Eq, PartialEq)]
#[serde(rename_all = "kebab-case", default, deny_unknown_fields)]
pub struct Config {
    /// Log configuration.
    pub log: LogConfig,

    /// Matcher configuration.
    pub matcher: MatcherConfig,

    /// Plugin configuration.
    pub plugin: PluginConfig,

    /// Provider configuration.
    pub provider: ProviderConfig,

    /// Global ignore configuration.
    pub global_ignore: IgnoreConfig,

    /// Ignore configuration per project.
    ///
    /// The project path must be specified as absolute path or a path relative to the home directory.
    pub project_ignore: HashMap<AbsPathBuf, IgnoreConfig>,
}

impl Config {
    pub fn ignore_config(&self, provider_id: &str, project_dir: &AbsPathBuf) -> &IgnoreConfig {
        self.provider.ignore.get(provider_id).unwrap_or_else(|| {
            self.project_ignore
                .get(project_dir)
                .unwrap_or(&self.global_ignore)
        })
    }

    pub fn provider_debounce(&self, provider_id: &str) -> u64 {
        const DEFAULT_DEBOUNCE: u64 = 200;

        self.provider
            .debounce
            .get(provider_id)
            .copied()
            .unwrap_or_else(|| {
                self.provider
                    .debounce
                    .get("*")
                    .copied()
                    .unwrap_or(DEFAULT_DEBOUNCE)
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_config() {
        let toml_content = r#"
          [log]
          max-level = "trace"
          log-file = "/tmp/clap.log"

          [matcher]
          tiebreak = "score,-begin,-end,-length"

          [plugin.cursorword]
          enable = true

          [provider.debounce]
          "*" = 200
          "files" = 100

          [global-ignore]
          ignore-file-path-pattern = ["test", "build"]

          # [project-ignore."~/src/github.com/subspace/subspace"]
          # ignore-comments = true

          [provider.ignore.dumb_jump]
          ignore-comments = true
"#;
        let user_config: Config =
            toml::from_str(toml_content).expect("Failed to deserialize config");

        assert_eq!(
            user_config,
            Config {
                log: LogConfig {
                    log_file: Some("/tmp/clap.log".to_string()),
                    max_level: "trace".to_string()
                },
                matcher: MatcherConfig {
                    tiebreak: "score,-begin,-end,-length".to_string()
                },
                plugin: PluginConfig {
                    cursorword: CursorWordConfig {
                        enable: true,
                        ..Default::default()
                    },
                    ..Default::default()
                },
                provider: ProviderConfig {
                    debounce: HashMap::from_iter([
                        ("*".to_string(), 200),
                        ("files".to_string(), 100)
                    ]),
                    ignore: HashMap::from([(
                        "dumb_jump".to_string(),
                        IgnoreConfig {
                            ignore_comments: true,
                            ..Default::default()
                        }
                    )]),
                    ..Default::default()
                },
                global_ignore: IgnoreConfig {
                    ignore_file_path_pattern: vec!["test".to_string(), "build".to_string()],
                    ..Default::default()
                },
                ..Default::default()
            }
        );
    }
}
