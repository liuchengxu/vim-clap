use crate::stdio_server::input::{AutocmdEventType, PluginEvent};
use crate::stdio_server::plugin::{
    Action, ActionType, ClapAction, ClapPlugin, PluginAction, PluginId, Toggle,
};
use crate::stdio_server::vim::Vim;
use anyhow::{anyhow, Result};
use linter::Diagnostic;
use parking_lot::RwLock;
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Debug, Clone)]
struct ShareableDiagnostics(Arc<RwLock<Vec<Diagnostic>>>);

impl ShareableDiagnostics {
    fn update(&self, new: Vec<Diagnostic>) {
        let mut diagnostics = self.0.write();
        *diagnostics = new;
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
        let mut diagnostics = lint_result.diagnostics;
        diagnostics.sort_by(|a, b| a.line_start.cmp(&b.line_start));
        let _ = self
            .vim
            .exec("clap#plugin#linter#show", (self.bufnr, &diagnostics));
        self.diagnostics.update(diagnostics);
        Ok(())
    }
}

impl LintResultHandler {}

#[derive(Debug, Clone)]
struct BufferLinterInfo {
    workspace: PathBuf,
    diagnostics: ShareableDiagnostics,
}

#[derive(Debug, Clone)]
pub struct LinterPlugin {
    vim: Vim,
    bufs: HashMap<usize, BufferLinterInfo>,
    toggle: Toggle,
}

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
        let handler =
            LintResultHandler::new(bufnr, self.vim.clone(), buf_linter_info.diagnostics.clone());

        linter::lint_in_background(
            PathBuf::from(source_file),
            &buf_linter_info.workspace,
            handler,
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
                use AutocmdEventType::{
                    BufDelete, BufEnter, BufWinLeave, BufWritePost, CursorMoved, InsertEnter,
                };

                if self.toggle.is_off() {
                    return Ok(());
                }

                let bufnr = params.parse_bufnr()?;

                match autocmd_event_type {
                    BufEnter => {
                        let filetype = self.vim.getbufvar::<String>(bufnr, "&filetype").await?;

                        const ENABLED_FILETYPES: &[&'static str] = &["rust"];
                        if !ENABLED_FILETYPES.contains(&filetype.as_str()) {
                            return Ok(());
                        }

                        let source_file = self.vim.bufabspath(bufnr).await?;
                        let source_file = PathBuf::from(source_file);
                        if let Some(workspace) =
                            paths::find_project_root(&source_file, &["Cargo.toml"])
                        {
                            let buf_linter_info = BufferLinterInfo {
                                workspace: workspace.to_path_buf(),
                                diagnostics: ShareableDiagnostics(Arc::new(
                                    RwLock::new(Vec::new()),
                                )),
                            };

                            self.lint_buffer(bufnr, &buf_linter_info).await?;

                            self.bufs.insert(bufnr, buf_linter_info);
                            return Ok(());
                        }
                    }
                    BufWritePost => {
                        let maybe_workspace = self.bufs.get(&bufnr).map(|info| &info.workspace);

                        if let Some(buf_linter_info) = self.bufs.get(&bufnr) {
                            self.lint_buffer(bufnr, buf_linter_info).await?;
                        }
                    }
                    CursorMoved => {}
                    _ => {}
                }

                Ok(())
            }
            PluginEvent::Action(plugin_action) => {
                let PluginAction { method, params: _ } = plugin_action;
                match method.as_str() {
                    Self::LINT => {
                        let source_file = self.vim.current_buffer_path().await?;
                        let source_file = PathBuf::from(source_file);
                        let Some(workspace) =
                            paths::find_project_root(&source_file, &["Cargo.toml"])
                        else {
                            return Ok(());
                        };

                        let mut diagnostics = linter::lint_file(&source_file, workspace)?;

                        diagnostics.sort_by(|a, b| a.line_start.cmp(&b.line_start));

                        tracing::debug!("{} diagnostics: {diagnostics:?}", diagnostics.len());

                        // let lnum = self.vim.line(".").await?;
                        // let current_diagnostics = diagnostics
                        // .iter()
                        // .filter(|d| d.line_start == lnum)
                        // .collect::<Vec<_>>();

                        let current_diagnostics = diagnostics;

                        if !current_diagnostics.is_empty() {
                            tracing::debug!("====== diagnostics: {current_diagnostics:?}");
                            if let Some(current_diagnostic) = current_diagnostics.first() {
                                self.vim.echo_info(current_diagnostic.human_message())?;
                            }

                            let bufnr = self.vim.bufnr("").await?;
                            self.vim
                                .exec("clap#plugin#linter#show", (bufnr, current_diagnostics))?;
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
