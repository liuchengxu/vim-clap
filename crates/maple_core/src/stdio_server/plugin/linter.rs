use crate::stdio_server::input::{AutocmdEventType, PluginEvent};
use crate::stdio_server::plugin::{
    Action, ActionType, ClapAction, ClapPlugin, PluginAction, PluginId, Toggle,
};
use crate::stdio_server::vim::Vim;
use anyhow::{anyhow, Result};
use linter::Diagnostic;
use once_cell::sync::Lazy;
use parking_lot::{Mutex, RwLock};
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::task::JoinHandle;

#[derive(Debug, Clone)]
struct ShareableDiagnostics {
    refreshed: Arc<AtomicBool>,
    diagnostics: Arc<RwLock<Vec<Diagnostic>>>,
}

impl Serialize for ShareableDiagnostics {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.diagnostics.read().serialize(serializer)
    }
}

impl ShareableDiagnostics {
    fn extend(&self, new: Vec<Diagnostic>) {
        let mut diagnostics = self.diagnostics.write();
        diagnostics.extend(new);
    }

    fn reset(&self) {
        self.refreshed.store(false, Ordering::SeqCst);
        let mut diagnostics = self.diagnostics.write();
        diagnostics.clear();
    }
}

#[derive(Clone)]
struct LintResultHandler {
    bufnr: usize,
    vim: Vim,
    shareable_diagnostics: ShareableDiagnostics,
}

impl LintResultHandler {
    fn new(bufnr: usize, vim: Vim, shareable_diagnostics: ShareableDiagnostics) -> Self {
        Self {
            bufnr,
            vim,
            shareable_diagnostics,
        }
    }
}

impl linter::HandleLintResult for LintResultHandler {
    fn handle_lint_result(&self, lint_result: linter::LintResult) -> std::io::Result<()> {
        let mut new_diagnostics = lint_result.diagnostics;
        new_diagnostics.sort_by(|a, b| a.line_start.cmp(&b.line_start));

        // Refresh if the first new diagnostics results arrive.
        if self
            .shareable_diagnostics
            .refreshed
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
        {
            let _ = self
                .vim
                .exec("clap#plugin#linter#refresh", (self.bufnr, &new_diagnostics));

            self.shareable_diagnostics.extend(new_diagnostics);
        } else {
            // Multiple linters can have an overlap over the diagnostics.
            let existing = self.shareable_diagnostics.diagnostics.read();
            let deduplicated_new = new_diagnostics
                .into_iter()
                .filter(|d| !existing.contains(d))
                .collect::<Vec<_>>();
            if !deduplicated_new.is_empty() {
                let _ = self
                    .vim
                    .exec("clap#plugin#linter#update", (self.bufnr, &deduplicated_new));
            }

            // Join the results from all the lint engines.
            self.shareable_diagnostics.extend(deduplicated_new);
        }

        Ok(())
    }
}

impl LintResultHandler {}

type LinterJob = JoinHandle<()>;

#[derive(Debug, Clone)]
struct BufferLinterInfo {
    workspace: PathBuf,
    source_file: PathBuf,
    diagnostics: ShareableDiagnostics,
    current_jobs: Arc<Mutex<Vec<LinterJob>>>,
}

impl BufferLinterInfo {
    fn new(workspace: PathBuf, source_file: PathBuf) -> Self {
        Self {
            workspace,
            source_file,
            diagnostics: ShareableDiagnostics {
                refreshed: Arc::new(AtomicBool::new(false)),
                diagnostics: Arc::new(RwLock::new(Vec::new())),
            },
            current_jobs: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

#[derive(Debug, Clone)]
pub struct LinterPlugin {
    vim: Vim,
    bufs: HashMap<usize, BufferLinterInfo>,
    toggle: Toggle,
}

#[derive(Debug, Clone)]
enum WorkspaceFinder {
    RootMarkers(&'static [&'static str]),
    /// Use the parent directory as the workspace if no explicit root markers.
    ParentOfSourceFile,
}

impl WorkspaceFinder {
    fn find_workspace<'a>(&'a self, source_file: &'a Path) -> Option<&Path> {
        match self {
            Self::RootMarkers(root_markers) => paths::find_project_root(source_file, root_markers),
            Self::ParentOfSourceFile => Some(source_file.parent().unwrap_or(source_file)),
        }
    }
}

static WORKSPACE_FINDERS: Lazy<HashMap<&str, WorkspaceFinder>> = Lazy::new(|| {
    HashMap::from_iter([
        ("rust", WorkspaceFinder::RootMarkers(&["Cargo.toml"])),
        ("go", WorkspaceFinder::RootMarkers(&["go.mod", ".git"])),
        ("sh", WorkspaceFinder::ParentOfSourceFile),
    ])
});

impl LinterPlugin {
    pub const ID: PluginId = PluginId::Linter;

    const LINT: &'static str = "linter/lint";
    const LINT_ACTION: Action = Action::callable(Self::LINT);

    const TOGGLE: &'static str = "linter/toggle";
    const TOGGLE_ACTION: Action = Action::callable(Self::TOGGLE);

    const ACTIONS: &[Action] = &[Self::LINT_ACTION, Self::TOGGLE_ACTION];

    pub fn new(vim: Vim) -> Self {
        Self {
            vim,
            bufs: HashMap::new(),
            toggle: Toggle::On,
        }
    }

    fn lint_buffer(&self, bufnr: usize, buf_linter_info: &BufferLinterInfo) -> Result<()> {
        buf_linter_info.diagnostics.reset();

        let mut current_jobs = buf_linter_info.current_jobs.lock();

        if !current_jobs.is_empty() {
            for job in &*current_jobs {
                job.abort();
            }
        }

        let jobs = linter::lint_in_background(
            buf_linter_info.source_file.clone(),
            &buf_linter_info.workspace,
            LintResultHandler::new(bufnr, self.vim.clone(), buf_linter_info.diagnostics.clone()),
        );

        *current_jobs = jobs;

        Ok(())
    }
}

impl ClapAction for LinterPlugin {
    fn actions(&self, _action_type: ActionType) -> &[Action] {
        Self::ACTIONS
    }
}

#[async_trait::async_trait]
impl ClapPlugin for LinterPlugin {
    fn id(&self) -> PluginId {
        Self::ID
    }

    async fn on_plugin_event(&mut self, plugin_event: PluginEvent) -> Result<()> {
        match plugin_event {
            PluginEvent::Autocmd((autocmd_event_type, params)) => {
                use AutocmdEventType::{
                    BufDelete, BufEnter, BufWritePost, TextChanged, TextChangedI,
                };

                if self.toggle.is_off() {
                    return Ok(());
                }

                let bufnr = params.parse_bufnr()?;

                tracing::debug!("======= event: {autocmd_event_type:?}");

                match autocmd_event_type {
                    BufEnter => {
                        let source_file = self.vim.bufabspath(bufnr).await?;
                        let source_file = PathBuf::from(source_file);

                        let filetype = self.vim.getbufvar::<String>(bufnr, "&filetype").await?;

                        let Some(workspace) =
                            WORKSPACE_FINDERS
                                .get(filetype.as_str())
                                .and_then(|workspace_finder| {
                                    workspace_finder.find_workspace(&source_file)
                                })
                        else {
                            return Ok(());
                        };

                        let buf_linter_info =
                            BufferLinterInfo::new(workspace.to_path_buf(), source_file);
                        self.lint_buffer(bufnr, &buf_linter_info)?;
                        self.bufs.insert(bufnr, buf_linter_info);

                        return Ok(());
                    }
                    BufWritePost | TextChanged | TextChangedI => {
                        if let Some(buf_linter_info) = self.bufs.get(&bufnr) {
                            self.lint_buffer(bufnr, buf_linter_info)?;
                        }
                    }
                    BufDelete => {
                        self.bufs.remove(&bufnr);
                    }
                    _ => {}
                }

                Ok(())
            }
            PluginEvent::Action(plugin_action) => {
                let PluginAction { method, params: _ } = plugin_action;
                match method.as_str() {
                    Self::LINT => {
                        let bufnr = self.vim.bufnr("").await?;
                        let source_file = self.vim.current_buffer_path().await?;
                        let source_file = PathBuf::from(source_file);

                        let filetype = self.vim.getbufvar::<String>(bufnr, "&filetype").await?;

                        let Some(workspace) =
                            WORKSPACE_FINDERS
                                .get(filetype.as_str())
                                .and_then(|workspace_finder| {
                                    workspace_finder.find_workspace(&source_file)
                                })
                        else {
                            return Ok(());
                        };

                        let mut diagnostics = linter::lint_file(&source_file, workspace)?;
                        diagnostics.sort_by(|a, b| a.line_start.cmp(&b.line_start));

                        let lnum = self.vim.line(".").await?;
                        let current_diagnostics = diagnostics
                            .into_iter()
                            .filter(|d| d.line_start == lnum)
                            .collect::<Vec<_>>();

                        if !current_diagnostics.is_empty() {
                            tracing::debug!("====== diagnostics: {current_diagnostics:?}");
                            if let Some(current_diagnostic) = current_diagnostics.first() {
                                self.vim.echo_info(current_diagnostic.human_message())?;
                            }

                            self.vim
                                .exec("clap#plugin#linter#refresh", (bufnr, current_diagnostics))?;
                        }
                    }
                    Self::TOGGLE => {
                        match self.toggle {
                            Toggle::On => {
                                // for bufnr in self.bufs.keys() {
                                // self.vim.exec("clap#plugin#git#clear_blame_info", [bufnr])?;
                                // }
                            }
                            Toggle::Off => {
                                let bufnr = self.vim.bufnr("").await?;

                                // self.on_cursor_moved(bufnr).await?;
                            }
                        }
                        self.toggle.switch();
                    }
                    unknown_action => return Err(anyhow!("Unknown action: {unknown_action:?}")),
                }

                Ok(())
            }
        }
    }
}
