use crate::stdio_server::input::{AutocmdEventType, PluginEvent};
use crate::stdio_server::plugin::{
    Action, ActionType, ClapAction, ClapPlugin, PluginAction, PluginId, Toggle,
};
use crate::stdio_server::vim::Vim;
use anyhow::{anyhow, Result};
use linter::Diagnostic;
use once_cell::sync::Lazy;
use parking_lot::RwLock;
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

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
    diagnostics: ShareableDiagnostics,
}

impl LintResultHandler {
    fn new(bufnr: usize, vim: Vim, diagnostics: ShareableDiagnostics) -> Self {
        Self {
            bufnr,
            vim,
            diagnostics,
        }
    }
}

impl linter::HandleLintResult for LintResultHandler {
    fn handle_lint_result(&self, lint_result: linter::LintResult) -> std::io::Result<()> {
        let mut new_diagnostics = lint_result.diagnostics;
        new_diagnostics.sort_by(|a, b| a.line_start.cmp(&b.line_start));

        // Refresh if the first new diagnostics results arrive.
        if self
            .diagnostics
            .refreshed
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
        {
            let _ = self
                .vim
                .exec("clap#plugin#linter#refresh", (self.bufnr, &new_diagnostics));
        } else {
            let _ = self
                .vim
                .exec("clap#plugin#linter#update", (self.bufnr, &new_diagnostics));
        }

        // Join the results from all the lint engines.
        self.diagnostics.extend(new_diagnostics);

        Ok(())
    }
}

impl LintResultHandler {}

#[derive(Debug, Clone)]
struct BufferLinterInfo {
    workspace: PathBuf,
    diagnostics: ShareableDiagnostics,
}

impl BufferLinterInfo {
    fn new(workspace: PathBuf) -> Self {
        Self {
            workspace,
            diagnostics: ShareableDiagnostics {
                refreshed: Arc::new(AtomicBool::new(false)),
                diagnostics: Arc::new(RwLock::new(Vec::new())),
            },
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
    pub fn find_workspace<'a>(&'a self, source_file: &'a Path) -> Option<&Path> {
        match self {
            Self::RootMarkers(root_markers) => paths::find_project_root(source_file, root_markers),
            Self::ParentOfSourceFile => Some(source_file.parent().unwrap_or(source_file)),
        }
    }
}

static SUPPORTED_LANGUAGE: Lazy<HashMap<&str, WorkspaceFinder>> = Lazy::new(|| {
    HashMap::from_iter([
        ("rust", WorkspaceFinder::RootMarkers(&["Cargo.toml"])),
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

    async fn lint_buffer(&self, bufnr: usize, buf_linter_info: &BufferLinterInfo) -> Result<()> {
        let source_file = self.vim.bufabspath(bufnr).await?;

        buf_linter_info.diagnostics.reset();

        linter::lint_in_background(
            PathBuf::from(source_file),
            &buf_linter_info.workspace,
            LintResultHandler::new(bufnr, self.vim.clone(), buf_linter_info.diagnostics.clone()),
        );

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
                use AutocmdEventType::{BufDelete, BufEnter, BufWritePost};

                if self.toggle.is_off() {
                    return Ok(());
                }

                let bufnr = params.parse_bufnr()?;

                match autocmd_event_type {
                    BufEnter => {
                        let source_file = self.vim.bufabspath(bufnr).await?;
                        let source_file = PathBuf::from(source_file);

                        let filetype = self.vim.getbufvar::<String>(bufnr, "&filetype").await?;

                        let Some(workspace) = SUPPORTED_LANGUAGE.get(filetype.as_str()).and_then(
                            |workspace_finder| workspace_finder.find_workspace(&source_file),
                        ) else {
                            return Ok(());
                        };

                        let buf_linter_info = BufferLinterInfo::new(workspace.to_path_buf());
                        self.lint_buffer(bufnr, &buf_linter_info).await?;
                        self.bufs.insert(bufnr, buf_linter_info);

                        return Ok(());
                    }
                    BufWritePost => {
                        if let Some(buf_linter_info) = self.bufs.get(&bufnr) {
                            self.lint_buffer(bufnr, buf_linter_info).await?;
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

                        let Some(workspace) = SUPPORTED_LANGUAGE.get(filetype.as_str()).and_then(
                            |workspace_finder| workspace_finder.find_workspace(&source_file),
                        ) else {
                            return Ok(());
                        };

                        let mut diagnostics = linter::lint_file(&source_file, workspace)?;
                        diagnostics.sort_by(|a, b| a.line_start.cmp(&b.line_start));

                        let current_diagnostics = diagnostics;

                        if !current_diagnostics.is_empty() {
                            tracing::debug!("====== diagnostics: {current_diagnostics:?}");
                            if let Some(current_diagnostic) = current_diagnostics.first() {
                                self.vim.echo_info(current_diagnostic.human_message())?;
                            }

                            let bufnr = self.vim.bufnr("").await?;
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
