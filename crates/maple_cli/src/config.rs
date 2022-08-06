use once_cell::sync::OnceCell;
use serde::{Deserialize, Serialize};

use crate::utils::PROJECT_DIRS;

pub fn config() -> &'static Config {
    static CONFIG: OnceCell<Config> = OnceCell::new();

    CONFIG.get_or_init(|| {
        // Linux: ~/.config/vimclap/config.toml
        let mut config_path = PROJECT_DIRS.config_dir().to_path_buf();
        config_path.push("config.toml");

        std::fs::read_to_string(config_path)
            .and_then(|contents| {
                toml::from_str(&contents).map_err(|err| {
                    tracing::debug!(?err, "Error while deserializing config.toml");
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
pub struct Config {
    #[serde(default)]
    pub provider: ProviderConfig,
}

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct ProviderConfig {
    pub dumb_jump: DumbJumpConfig,
    pub grep2: Grep2Config,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct DumbJumpConfig {
    pub ignore_files_not_git_tracked: bool,
    pub ignore_pattern_file_path: Option<String>,
}

impl Default for DumbJumpConfig {
    fn default() -> Self {
        Self {
            ignore_files_not_git_tracked: true,
            ignore_pattern_file_path: None,
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Grep2Config {
    pub ignore_comment_line: bool,
    // TODO: project-wise ignore pattern.
    /// Ignore the results from the files whose name contains this pattern.
    pub ignore_pattern_file_name: Option<String>,
    /// Ignore the results from the files whose path contains this pattern.
    pub ignore_pattern_file_path: Option<String>,
}

impl Default for Grep2Config {
    fn default() -> Self {
        Self {
            ignore_comment_line: true,
            ignore_pattern_file_name: None,
            ignore_pattern_file_path: None,
        }
    }
}

#[test]
fn test_config_serde() {
    let toml_content = r#"
          [provider.grep2]
          # Invalid entry will be ignored simply.
          ignore_pattern_file_path_foo = "test"
"#;

    let test_config: Config = toml::from_str(toml_content).unwrap();

    println!("{test_config:?}");

    println!("{}", toml::to_string(&test_config).unwrap());

    println!("User config\n{:?}", config());
}
