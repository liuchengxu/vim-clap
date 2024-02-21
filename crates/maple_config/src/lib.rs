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

fn load_config(
    specified_config_file: Option<PathBuf>,
) -> (Config, PathBuf, Option<toml::de::Error>) {
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

    (loaded_config, config_file, maybe_config_err)
}

pub fn load_config_on_startup(
    specified_config_file: Option<PathBuf>,
) -> (&'static Config, Option<toml::de::Error>) {
    let (loaded_config, config_file, maybe_config_err) = load_config(specified_config_file);

    CONFIG_FILE
        .set(config_file)
        .expect("Failed to initialize Config file");

    CONFIG
        .set(loaded_config)
        .expect("Failed to initialize Config");

    (config(), maybe_config_err)
}

pub fn config() -> &'static Config {
    CONFIG.get_or_init(|| load_config(None).0)
}

pub fn config_file() -> &'static PathBuf {
    CONFIG_FILE.get().expect("Config file uninitialized")
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
#[serde(rename_all = "kebab-case", default, deny_unknown_fields)]
pub struct MatcherConfig {
    /// Specify how the results are sorted.
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

#[derive(Serialize, Deserialize, Debug, PartialEq)]
#[serde(rename_all = "kebab-case", default, deny_unknown_fields)]
pub struct LogConfig {
    /// Specify the log file path.
    ///
    /// This path must be an absolute path.
    pub log_file: Option<String>,

    /// Specify the max log level.
    pub max_level: String,

    /// Specify the log target to enable more detailed logging.
    ///
    /// Particularly useful for the debugging purpose.
    ///
    /// ```toml
    /// [log]
    /// log-target = "maple_core::stdio_server=trace,rpc=debug"
    /// ```
    pub log_target: String,
}

impl Default for LogConfig {
    fn default() -> Self {
        Self {
            log_file: None,
            max_level: "debug".into(),
            log_target: "".into(),
        }
    }
}

/// Cursorword plugin.
#[derive(Serialize, Deserialize, Debug, PartialEq)]
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

/// Markdown plugin.
#[derive(Serialize, Deserialize, Debug, Default, PartialEq)]
#[serde(rename_all = "kebab-case", default, deny_unknown_fields)]
pub struct MarkdownPluginConfig {
    /// Whether to enable this plugin.
    pub enable: bool,
}

#[derive(Serialize, Deserialize, Debug, Default, PartialEq)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub enum FilePathStyle {
    /// Display each component in path in one segment.
    ///
    /// crates > maple_core > src > stdio_server > plugin > ctags.rs
    #[default]
    OneSegmentPerComponent,

    /// Display the full path in one segment.
    ///
    /// crates/maple_core/src/stdio_server/plugin/ctags.rs
    FullPath,
}

/// Winbar config. (Neovim only)
#[derive(Serialize, Deserialize, Debug, PartialEq)]
#[serde(rename_all = "kebab-case", default, deny_unknown_fields)]
pub struct WinbarConfig {
    /// Whether to enable the winbar.
    pub enable: bool,

    /// Specify the display style for the file path in winbar.
    pub file_path_style: FilePathStyle,
}

impl Default for WinbarConfig {
    fn default() -> Self {
        Self {
            enable: false,
            file_path_style: FilePathStyle::OneSegmentPerComponent,
        }
    }
}

/// Ctags plugin.
#[derive(Serialize, Deserialize, Debug, PartialEq)]
#[serde(rename_all = "kebab-case", default, deny_unknown_fields)]
pub struct CtagsPluginConfig {
    /// Whether to enable this plugin.
    pub enable: bool,

    /// Disable this plugin if the file size exceeds the max size limit.
    ///
    /// By default the max file size limit is 4MiB.
    pub max_file_size: u64,
}

impl Default for CtagsPluginConfig {
    fn default() -> Self {
        Self {
            enable: false,
            max_file_size: 4 * 1024 * 1024,
        }
    }
}

/// Git plugin.
#[derive(Serialize, Deserialize, Debug, PartialEq)]
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

/// Colorizer plugin.
#[derive(Serialize, Deserialize, Debug, Default, PartialEq)]
#[serde(rename_all = "kebab-case", default, deny_unknown_fields)]
pub struct ColorizerPluginConfig {
    /// Whether to enable this plugin.
    pub enable: bool,
}

/// Linter plugin.
#[derive(Serialize, Deserialize, Debug, Default, PartialEq)]
#[serde(rename_all = "kebab-case", default, deny_unknown_fields)]
pub struct LinterPluginConfig {
    /// Whether to enable this plugin.
    pub enable: bool,
}

#[derive(Clone, Serialize, Deserialize, Debug, Default, PartialEq)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct LanguageConfig {
    /// c-sharp, rust, tsx
    pub name: String,

    /// List of `&filetype` corresponding to this language.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub file_types: Vec<String>,

    /// List of `&filetype` corresponding to this language.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub file_extensions: Vec<String>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub line_comments: Vec<String>,

    /// these indicate project roots <.git, Cargo.toml>
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub root_markers: Vec<String>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub language_servers: Vec<String>,
}

impl LanguageConfig {
    pub fn merge(&mut self, other: Self) {
        let merge_vec = |v: &mut Vec<String>, other: Vec<String>| {
            v.extend(other);
            v.sort();
            v.dedup();
        };

        let Self {
            file_types,
            file_extensions,
            line_comments,
            root_markers,
            language_servers,
            ..
        } = other;

        merge_vec(&mut self.file_types, file_types);
        merge_vec(&mut self.file_extensions, file_extensions);
        merge_vec(&mut self.line_comments, line_comments);
        merge_vec(&mut self.root_markers, root_markers);
        merge_vec(&mut self.language_servers, language_servers);
    }
}

/// LSP plugin.
#[derive(Serialize, Deserialize, Debug, Default, PartialEq)]
#[serde(rename_all = "kebab-case", default, deny_unknown_fields)]
pub struct LspPluginConfig {
    /// Whether to enable this plugin.
    pub enable: bool,

    /// Whether to include the declaration when invoking goto-reference.
    pub include_declaration: bool,

    /// Specifies custom languages that are not built into vim-clap.
    ///
    /// If a language is not included in the default languages supported by vim-clap,
    /// you can specify it here. Note that for languages not listed in the default
    /// configuration (check out the full list of supported languages in `languages.toml`),
    /// you need to provide associated language server configurations as well.
    ///
    /// # Example
    ///
    /// ```toml
    /// [[plugin.lsp.language]]
    /// name = "erlang"
    /// file-types = ["erlang"]
    /// root-markers = ["rebar.config"]
    /// language-servers = ["erlang-ls"]
    ///
    /// [plugin.lsp.language-server.erlang-ls]
    /// command = "erlang_ls"
    /// args = ["--transport", "stdio"]
    /// ```
    pub language: Vec<LanguageConfig>,

    /// Specify language server configurations.
    ///
    /// # Example
    ///
    /// ```toml
    /// [plugin.lsp.language-server.rust-analyzer]
    /// procMacro.enable = false
    /// procMacro.attributes.enable = false
    /// diagnostics.disabled = [ "unresolved-proc-macro" ]
    /// ```
    pub language_server: HashMap<String, toml::Value>,
}

/// Syntax plugin.
#[derive(Serialize, Deserialize, Debug, Default, PartialEq)]
#[serde(rename_all = "kebab-case", default, deny_unknown_fields)]
pub struct SyntaxPluginConfig {
    /// Specify the strategy of tree-sitter rendering.
    ///
    /// The default strategy is to render the entire buffer until the
    /// file size exceeds 256 KiB.
    ///
    ///
    /// Possible values:
    /// - `visual-lines`: Always render the visual lines only.
    /// - `entire-buffer-up-to-limit`: Render the entire buffer until
    /// the buffer size exceeds the size limit (in bytes).
    ///
    /// # Example
    ///
    /// ```toml
    /// [plugin.syntax.render-strategy]
    /// strategy = "visual-lines"
    /// ```
    pub render_strategy: RenderStrategy,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
#[serde(
    tag = "strategy",
    content = "file-size-limit",
    rename_all = "kebab-case",
    deny_unknown_fields
)]
pub enum RenderStrategy {
    /// Render only the visual lines.
    VisualLines,

    /// Render the entire buffer until the file size limit is reached.
    ///
    /// This strategy renders the complete buffer until the file size
    /// exceeds the specified limit. It's not recommended to always render
    /// large buffers directly due to potential performance issues.
    /// For smaller buffers, this strategy enhances the user experience.
    EntireBufferUpToLimit(usize),
}

impl Default for RenderStrategy {
    fn default() -> Self {
        Self::EntireBufferUpToLimit(256 * 1024)
    }
}

#[derive(Serialize, Deserialize, Debug, Default, PartialEq)]
#[serde(rename_all = "kebab-case", default, deny_unknown_fields)]
pub struct PluginConfig {
    pub colorizer: ColorizerPluginConfig,
    pub cursorword: CursorWordConfig,
    pub ctags: CtagsPluginConfig,
    pub git: GitPluginConfig,
    pub linter: LinterPluginConfig,
    pub lsp: LspPluginConfig,
    pub markdown: MarkdownPluginConfig,
    pub syntax: SyntaxPluginConfig,
}

/// Represents configuration options for ignoring certain files/folders/patterns when searching.
#[derive(Serialize, Deserialize, Debug, Default, PartialEq)]
#[serde(rename_all = "kebab-case", default, deny_unknown_fields)]
pub struct IgnoreConfig {
    /// Whether to ignore the comment line when applicable.
    pub ignore_comments: bool,

    /// Whether to include only results from files tracked by git, if in a git repository.
    pub git_tracked_only: bool,

    /// Patterns for ignoring files based on their names.
    ///
    /// For instance, to exclude results from files whose names contain "test" for
    /// the dumb_jump provider:
    ///
    /// ```toml
    /// [provider.provider-ignores.dumb_jump]
    /// ignore-file-name-pattern = ["test"]
    /// ```
    pub ignore_file_name_pattern: Vec<String>,

    /// Patterns for ignoring files based on their paths.
    ///
    /// For instance, to exclude the results from the folder whose names contain `test`
    /// in the project `~/src/github.com/vim/vim`:
    ///
    /// ```toml
    /// [provider.project-ignores."~/src/github.com/vim/vim"]
    /// ignore-file-path-pattern = ["test"]
    /// ```
    pub ignore_file_path_pattern: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, Default, PartialEq)]
#[serde(rename_all = "kebab-case", default, deny_unknown_fields)]
pub struct ProviderConfig {
    /// Whether to share the input history among providers.
    pub share_input_history: bool,

    /// Specifies the maximum number of items to be displayed
    /// in the results window.
    pub max_display_size: Option<usize>,

    /// Specify the syntax highlight engine for the provider preview.
    ///
    /// Possible values: `vim`, `sublime-syntax` and `tree-sitter`
    pub preview_highlight_engine: HighlightEngine,

    /// Specify the theme for the highlight engine.
    ///
    /// If not found, the default theme (`Visual Studio Dark+`) is used
    /// when the engine is [`HighlightEngine::SublimeSyntax`],
    pub sublime_syntax_color_scheme: Option<String>,

    /// Ignore configuration per project, with paths specified as
    /// absolute path or relative to the home directory.
    pub project_ignores: HashMap<AbsPathBuf, IgnoreConfig>,

    /// Ignore configuration per provider.
    ///
    /// There are multiple ignore settings, with priorities as follows:
    /// `provider_ignores` > `provider_ignores` > `global_ignore`
    pub provider_ignores: HashMap<String, IgnoreConfig>,

    /// Delay in milliseconds before handling the the user query.
    ///
    /// When the delay is set not-zero, some intermediate inputs
    /// may be dropped if user types too fast.
    ///
    /// By default the debounce is set to 200ms to all providers.
    ///
    /// # Example
    ///
    /// ```toml
    /// [provider.debounce]
    /// # Set debounce to 100ms for files provider specifically.
    /// "files" = 100
    /// ```
    pub debounce: HashMap<String, u64>,
}

#[derive(Serialize, Deserialize, Debug, Default, PartialEq)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub enum HighlightEngine {
    SublimeSyntax,
    TreeSitter,
    #[default]
    Vim,
}

#[derive(Serialize, Deserialize, Debug, Default, PartialEq)]
#[serde(rename_all = "kebab-case", default, deny_unknown_fields)]
pub struct Config {
    /// Log configuration.
    pub log: LogConfig,

    /// Winbar configuration.
    pub winbar: WinbarConfig,

    /// Matcher configuration.
    pub matcher: MatcherConfig,

    /// Plugin configuration.
    pub plugin: PluginConfig,

    /// Provider (fuzzy picker) configuration.
    pub provider: ProviderConfig,

    /// Global ignore configuration.
    pub global_ignore: IgnoreConfig,
}

impl Config {
    /// Retrieves the `IgnoreConfig` for a given provider and project directory.
    ///
    /// If a specific `provider_id` is provided, it looks up the configuration in the provider-specific
    /// ignores. If not found, it falls back to checking the project-specific ignores based on the
    /// provided `project_dir`. If neither is found, it defaults to the global ignore configuration.
    pub fn ignore_config(&self, provider_id: &str, project_dir: &AbsPathBuf) -> &IgnoreConfig {
        self.provider
            .provider_ignores
            .get(provider_id)
            .or_else(|| self.provider.project_ignores.get(project_dir))
            .unwrap_or(&self.global_ignore)
    }

    /// Retrieves the debounce configuration for a specific provider or falls back to a default value.
    pub fn provider_debounce(&self, provider_id: &str) -> u64 {
        const DEFAULT_DEBOUNCE: u64 = 200;

        self.provider
            .debounce
            .get(provider_id)
            .or_else(|| self.provider.debounce.get("*"))
            .copied()
            .unwrap_or(DEFAULT_DEBOUNCE)
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

          # [provider.project-ignore."~/src/github.com/subspace/subspace"]
          # ignore-comments = true

          [provider.provider-ignores.dumb_jump]
          ignore-comments = true
"#;
        let user_config: Config =
            toml::from_str(toml_content).expect("Failed to deserialize config");

        assert_eq!(
            user_config,
            Config {
                log: LogConfig {
                    log_file: Some("/tmp/clap.log".to_string()),
                    max_level: "trace".to_string(),
                    ..Default::default()
                },
                winbar: WinbarConfig {
                    enable: false,
                    file_path_style: FilePathStyle::OneSegmentPerComponent
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
                    provider_ignores: HashMap::from([(
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
            }
        );
    }

    #[test]
    fn test_config_deserialize() {
        let config = Config::default();
        toml::to_string_pretty(&config).expect("Deserialize config is okay");
    }
}
