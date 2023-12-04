use crate::stdio_server::input::{AutocmdEvent, AutocmdEventType};
use crate::stdio_server::plugin::{ActionRequest, ClapPlugin, PluginError, Toggle};
use crate::stdio_server::vim::Vim;
use crate::tools::git::{parse_blame_info, GitError, GitRepo, Summary};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

fn in_git_repo(filepath: &Path) -> Option<&Path> {
    filepath
        .exists()
        .then(|| paths::find_git_root(filepath))
        .flatten()
}

#[derive(Debug, Clone, maple_derive::ClapPlugin)]
#[clap_plugin(
  id = "git",
  actions = [
    "blame",
    "diff-summary",
    "hunk-modifications",
    "open-permalink-in-browser",
    "toggle",
])]
pub struct Git {
    vim: Vim,
    bufs: HashMap<usize, (PathBuf, GitRepo)>,
    git_summary: HashMap<usize, Summary>,
    toggle: Toggle,
}

impl Git {
    pub fn new(vim: Vim) -> Self {
        Self {
            vim,
            bufs: HashMap::new(),
            git_summary: HashMap::new(),
            toggle: Toggle::On,
        }
    }

    async fn try_track_buffer(&mut self, bufnr: usize) -> Result<(), PluginError> {
        if self.bufs.contains_key(&bufnr) {
            return Ok(());
        }

        let buf_path = self.vim.current_buffer_path().await?;

        let filepath = PathBuf::from(buf_path);

        if let Some(git_root) = in_git_repo(&filepath) {
            let git = GitRepo::init(git_root.to_path_buf())?;
            if git.is_tracked(&filepath)? {
                self.bufs.insert(bufnr, (filepath, git));
                return Ok(());
            } else {
                return Err(GitError::Untracked.into());
            }
        }

        Ok(())
    }

    async fn on_cursor_moved(&self, bufnr: usize) -> Result<(), PluginError> {
        if let Some((filepath, git)) = self.bufs.get(&bufnr) {
            let maybe_blame_info = self.cursor_line_blame_info(git, filepath).await?;
            if let Some(blame_info) = maybe_blame_info {
                self.vim.exec(
                    "clap#plugin#git#show_cursor_blame_info",
                    (bufnr, blame_info),
                )?;
            }
        }
        Ok(())
    }

    fn update_diff_summary(&mut self, bufnr: usize) -> Result<(), PluginError> {
        if let Some((filepath, git)) = self.bufs.get(&bufnr) {
            let diff_summary = git.get_diff_summary(filepath, None)?;

            if let Some(old_summary) = self.git_summary.get(&bufnr) {
                if diff_summary.eq(old_summary) {
                    return Ok(());
                }
            }

            self.vim.setbufvar(
                bufnr,
                "clap_git",
                serde_json::json!({
                  "summary": [diff_summary.added, diff_summary.modified, diff_summary.removed]
                }),
            )?;
            self.git_summary.insert(bufnr, diff_summary);
        }
        Ok(())
    }

    async fn cursor_line_blame_info(
        &self,
        git: &GitRepo,
        filepath: &Path,
    ) -> Result<Option<String>, PluginError> {
        let relative_path = filepath.strip_prefix(&git.repo)?;

        let lnum = self.vim.line(".").await?;

        let stdout = if self.vim.bufmodified("").await? {
            let lines = self.vim.getbufline("", 1, "$").await?;
            git.fetch_blame_output_with_lines(relative_path, lnum, lines)?
        } else {
            git.fetch_blame_output(relative_path, lnum)?
        };

        if let Some(blame_info) = parse_blame_info(stdout) {
            return Ok(Some(
                blame_info
                    .display(&git.user_name)
                    .ok_or_else(|| {
                        PluginError::Other("failed to fetch line blame info".to_string())
                    })?
                    .to_string(),
            ));
        }

        Ok(None)
    }

    async fn show_blame_info(&self) -> Result<(), PluginError> {
        let buf_path = self.vim.current_buffer_path().await?;
        let filepath = PathBuf::from(buf_path);

        let Some(git_root) = in_git_repo(&filepath) else {
            return Ok(());
        };

        if let Ok(Some(blame_info)) = self
            .cursor_line_blame_info(&GitRepo::init(git_root.to_path_buf())?, &filepath)
            .await
        {
            self.vim.echo_info(blame_info)?;
        }

        Ok(())
    }

    async fn open_permalink_in_browser(&self) -> Result<(), PluginError> {
        let buf_path = self.vim.current_buffer_path().await?;
        let filepath = PathBuf::from(buf_path);

        let Some(git_root) = in_git_repo(&filepath) else {
            return Ok(());
        };

        let git = GitRepo::init(git_root.to_path_buf())?;

        let relative_path = filepath.strip_prefix(&git.repo)?;

        let stdout = git.fetch_origin_url()?;
        let remote_url = stdout.trim();

        // https://github.com/liuchengxu/vim-clap{.git}
        let remote_url = remote_url.strip_suffix(".git").unwrap_or(remote_url);

        let Ok(stdout) = git.fetch_rev_parse("HEAD") else {
            return Ok(());
        };

        let Some(rev) = stdout.split('\n').next() else {
            return Ok(());
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

        Ok(())
    }
}

#[async_trait::async_trait]
impl ClapPlugin for Git {
    #[maple_derive::subscriptions]
    async fn handle_autocmd(&mut self, autocmd: AutocmdEvent) -> Result<(), PluginError> {
        use AutocmdEventType::{
            BufDelete, BufEnter, BufLeave, BufWritePost, CursorMoved, InsertEnter,
        };

        if self.toggle.is_off() {
            return Ok(());
        }

        let (autocmd_event_type, params) = autocmd;
        let bufnr = params.parse_bufnr()?;

        match autocmd_event_type {
            BufEnter => {
                self.try_track_buffer(bufnr).await?;
                self.on_cursor_moved(bufnr).await?;
                self.update_diff_summary(bufnr)?;
            }
            BufWritePost => {
                self.update_diff_summary(bufnr)?;
            }
            BufDelete => {
                self.bufs.remove(&bufnr);
                self.git_summary.remove(&bufnr);
            }
            InsertEnter | BufLeave => {
                self.vim.exec("clap#plugin#git#clear_blame_info", [bufnr])?;
            }
            CursorMoved => {
                self.on_cursor_moved(bufnr).await?;
                self.update_diff_summary(bufnr)?;
            }
            event => return Err(PluginError::UnhandledEvent(event)),
        }

        Ok(())
    }

    async fn handle_action(&mut self, action: ActionRequest) -> Result<(), PluginError> {
        let ActionRequest { method, params: _ } = action;

        match self.parse_action(method)? {
            GitAction::Toggle => {
                match self.toggle {
                    Toggle::On => {
                        for bufnr in self.bufs.keys() {
                            self.vim.exec("clap#plugin#git#clear_blame_info", [bufnr])?;
                        }
                    }
                    Toggle::Off => {
                        let bufnr = self.vim.bufnr("").await?;

                        self.on_cursor_moved(bufnr).await?;
                    }
                }
                self.toggle.switch();
            }
            GitAction::OpenPermalinkInBrowser => {
                self.open_permalink_in_browser().await?;
            }
            GitAction::Blame => self.show_blame_info().await?,
            GitAction::DiffSummary => {
                let buf_path = self.vim.current_buffer_path().await?;
                let filepath = PathBuf::from(buf_path);

                let Some(git_root) = in_git_repo(&filepath) else {
                    return Ok(());
                };

                let git = GitRepo::init(git_root.to_path_buf())?;

                let summary = git.get_diff_summary(&filepath, None)?;
                self.vim.echo_info(format!("summary: {summary:?}"))?;
            }
            GitAction::HunkModifications => {
                let buf_path = self.vim.current_buffer_path().await?;
                let filepath = PathBuf::from(buf_path);

                let Some(git_root) = in_git_repo(&filepath) else {
                    return Ok(());
                };

                let git = GitRepo::init(git_root.to_path_buf())?;

                let modifications = git.get_hunk_modifications(&filepath, None)?;
                self.vim.echo_info(format!("{modifications:?}"))?;
            }
        }

        Ok(())
    }
}
