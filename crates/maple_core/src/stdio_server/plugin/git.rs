use crate::stdio_server::plugin::{ActionType, ClapPlugin, PluginAction, PluginEvent, PluginId};
use crate::stdio_server::vim::Vim;
use anyhow::{anyhow, Result};
use std::path::Path;

#[derive(Debug, Clone)]
pub struct GitPlugin {
    vim: Vim,
}

impl GitPlugin {
    const OPEN_CURRENT_LINE_IN_BROWSER: &'static str = "git/open-current-line-in-browser";
    const BLAME: &'static str = "git/blame";

    pub const ID: PluginId = PluginId::Git;
    pub const ACTIONS: &[&'static str] = &[Self::OPEN_CURRENT_LINE_IN_BROWSER, Self::BLAME];

    pub fn new(vim: Vim) -> Self {
        Self { vim }
    }
}

#[async_trait::async_trait]
impl ClapPlugin for GitPlugin {
    fn id(&self) -> PluginId {
        Self::ID
    }

    fn actions(&self, _action_type: ActionType) -> &[&'static str] {
        Self::ACTIONS
    }

    async fn on_plugin_event(&mut self, plugin_event: PluginEvent) -> Result<()> {
        match plugin_event {
            PluginEvent::Autocmd(_) => Ok(()),
            PluginEvent::Action(plugin_action) => {
                let PluginAction { action, params: _ } = plugin_action;
                match action.as_str() {
                    Self::OPEN_CURRENT_LINE_IN_BROWSER => {
                        let buf_path = self.vim.current_buffer_path().await?;
                        let filepath = Path::new(&buf_path);

                        let Some(git_root) = filepath
                            .exists()
                            .then(|| crate::paths::find_git_root(&filepath))
                            .flatten()
                        else {
                            return Ok(());
                        };

                        let relative_path = filepath.strip_prefix(git_root)?;

                        let output = std::process::Command::new("git")
                            .current_dir(git_root)
                            .arg("remote")
                            .arg("-v")
                            .stderr(std::process::Stdio::inherit())
                            .output()?;

                        let stdout = String::from_utf8_lossy(&output.stdout);
                        let Some(remote_url) = stdout
                            .split('\n')
                            .find(|line| line.starts_with("origin"))
                            .and_then(|origin_line| origin_line.split_whitespace().nth(1))
                        else {
                            return Ok(());
                        };

                        // https://github.com/liuchengxu/vim-clap{.git}
                        let remote_url = remote_url.strip_suffix(".git").unwrap_or(remote_url);

                        let output = std::process::Command::new("git")
                            .current_dir(git_root)
                            .arg("rev-parse")
                            .arg("HEAD")
                            .stderr(std::process::Stdio::inherit())
                            .output()?;

                        let stdout = String::from_utf8_lossy(&output.stdout);
                        let Some(rev) = stdout.split('\n').next() else {
                            return Ok(())
                        };

                        let lnum = self.vim.line(".").await?;
                        let commit_url = format!(
                            "{remote_url}/blob/{rev}/{}#L{lnum}",
                            relative_path.display()
                        );

                        if let Err(e) = webbrowser::open(&commit_url) {
                            self.vim
                                .echo_warn(format!("Failed to open {commit_url}: {e:?}"))?;
                        }
                    }
                    Self::BLAME => {
                        let buf_path = self.vim.current_buffer_path().await?;
                        let filepath = Path::new(&buf_path);

                        let Some(git_root) = filepath
                            .exists()
                            .then(|| crate::paths::find_git_root(&filepath))
                            .flatten()
                        else {
                            return Ok(());
                        };

                        let relative_path = filepath.strip_prefix(git_root)?;

                        let lnum = self.vim.line(".").await?;

                        let output = std::process::Command::new("git")
                            .current_dir(git_root)
                            .arg("blame")
                            .arg(format!("-L{lnum},{lnum}"))
                            .arg("--")
                            .arg(relative_path)
                            .stderr(std::process::Stdio::inherit())
                            .output()?;

                        let stdout = String::from_utf8_lossy(&output.stdout);

                        self.vim.echo_info(stdout)?;
                    }
                    unknown_action => return Err(anyhow!("Unknown action: {unknown_action:?}")),
                }

                Ok(())
            }
        }
    }
}
