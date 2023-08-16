use crate::stdio_server::plugin::{ActionType, ClapPlugin, PluginAction, PluginEvent, PluginId};
use crate::stdio_server::vim::Vim;
use anyhow::{anyhow, Result};
use chrono::{TimeZone, Utc};
use itertools::Itertools;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Stdio;

#[derive(Debug, Clone)]
pub struct GitPlugin {
    vim: Vim,
}

fn fetch_rev_parse(git_root: &Path, arg: &str) -> Result<String> {
    let output = std::process::Command::new("git")
        .current_dir(git_root)
        .arg("rev-parse")
        .arg(arg)
        .stderr(std::process::Stdio::inherit())
        .output()?;

    Ok(String::from_utf8(output.stdout)?)
}

fn fetch_user_name(git_root: &Path) -> Result<String> {
    let output = std::process::Command::new("git")
        .current_dir(git_root)
        .arg("config")
        .arg("user.name")
        .stderr(std::process::Stdio::inherit())
        .output()?;

    Ok(String::from_utf8(output.stdout)?)
}

fn fetch_blame_info(git_root: &Path, relative_path: &Path, lnum: usize) -> Result<Vec<u8>> {
    let output = std::process::Command::new("git")
        .current_dir(git_root)
        .arg("blame")
        .arg("--porcelain")
        .arg("--incremental")
        .arg(format!("-L{lnum},{lnum}"))
        .arg("--")
        .arg(relative_path)
        .stderr(std::process::Stdio::inherit())
        .output()?;

    Ok(output.stdout)
}

// git blame --contents - -L 100,+1 --line-porcelain crates/maple_core/src/stdio_server/plugin/git.rs
fn fetch_blame_info_with_lines(
    git_root: &Path,
    relative_path: &Path,
    lnum: usize,
    lines: Vec<String>,
) -> Result<Vec<u8>> {
    let mut p = std::process::Command::new("git")
        .current_dir(git_root)
        .arg("blame")
        .arg("--contents")
        .arg("-")
        .arg("-L")
        .arg(format!("{lnum},+1"))
        .arg("--line-porcelain")
        .arg(relative_path)
        .stdin(Stdio::piped())
        .spawn()?;

    let lines = lines.into_iter().join("\n");
    let stdin = p
        .stdin
        .as_mut()
        .ok_or_else(|| anyhow!("stdin unavailable"))?;
    stdin.write_all(lines.as_bytes())?;

    let output = p.wait_with_output()?;

    if output.status.success() {
        Ok(output.stdout)
    } else {
        Err(anyhow!(
            "Child process errors out: {}",
            String::from_utf8_lossy(&output.stderr)
        ))
    }
}

fn parse_blame_output(stdout: Vec<u8>) -> Result<(String, i64, String)> {
    let stdout = String::from_utf8_lossy(&stdout);

    let mut author = None;
    let mut author_time = None;
    let mut summary = None;

    for line in stdout.split('\n') {
        if let Some((k, v)) = line.split_once(' ') {
            match k {
                "author" => {
                    author.replace(v);
                }
                "author-time" => {
                    author_time.replace(v);
                }
                "summary" => {
                    summary.replace(v);
                }
                _ => {}
            }
        }

        if let (Some(author), Some(author_time), Some(summary)) = (author, author_time, summary) {
            let time = author_time.parse::<i64>()?;

            return Ok((author.to_owned(), time, summary.to_string()));
        }
    }

    Err(anyhow!("blame digest not found in output"))
}

impl GitPlugin {
    const BLAME: &'static str = "git/blame";
    const OPEN_CURRENT_LINE_IN_BROWSER: &'static str = "git/open-current-line-in-browser";

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
                            .then(|| crate::paths::find_git_root(filepath))
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

                        let stdout = fetch_rev_parse(git_root, "HEAD")?;
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
                        let filepath = PathBuf::from(buf_path);

                        let Some(git_root) = filepath
                            .exists()
                            .then(|| crate::paths::find_git_root(&filepath))
                            .flatten()
                        else {
                            return Ok(());
                        };

                        let relative_path = filepath.strip_prefix(git_root)?;

                        let lnum = self.vim.line(".").await?;

                        let stdout = if self.vim.bufmodified("").await? {
                            let lines = self.vim.getbufline("", 1, "$").await?;
                            fetch_blame_info_with_lines(git_root, relative_path, lnum, lines)?
                        } else {
                            fetch_blame_info(git_root, relative_path, lnum)?
                        };

                        let (author, author_time, summary) = parse_blame_output(stdout)?;

                        if author == "Not Committed Yet" {
                            self.vim.echo_info(author)?;
                        } else {
                            let user_name = fetch_user_name(git_root)?;
                            let time =
                                Utc.timestamp_opt(author_time, 0).single().ok_or_else(|| {
                                    anyhow!("Failed to parse timestamp {author_time}")
                                })?;
                            if user_name == author {
                                self.vim.echo_info(format!("(You {time}) {summary}"))?;
                            } else {
                                self.vim.echo_info(format!("({author} {time}) {summary}"))?;
                            }
                        }
                    }

                    unknown_action => return Err(anyhow!("Unknown action: {unknown_action:?}")),
                }

                Ok(())
            }
        }
    }
}
