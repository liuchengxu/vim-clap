#![allow(clippy::enum_variant_names)]

use crate::stdio_server::input::{AutocmdEvent, AutocmdEventType, PluginAction};
use crate::stdio_server::plugin::{ClapPlugin, PluginError, Toggle};
use crate::stdio_server::vim::Vim;
use maple_markdown::toc::{find_toc_range, generate_toc};
use maple_markdown::Message;
use serde_json::json;

/// Active preview server state for the currently previewed markdown file
#[derive(Debug)]
struct ActivePreview {
    /// The buffer number currently being displayed in the preview
    bufnr: usize,
    /// Port the preview server is running on
    port: u16,
    msg_tx: tokio::sync::watch::Sender<Message>,
    shutdown_tx: tokio::sync::oneshot::Sender<()>,
}

#[derive(Debug, maple_derive::ClapPlugin)]
#[clap_plugin(
  id = "markdown",
  actions = [
    "generateToc",
    "updateToc",
    "deleteToc",
    "previewInBrowser",
])]
pub struct Markdown {
    vim: Vim,
    /// Single active preview (only one file can be previewed at a time)
    active_preview: Option<ActivePreview>,
    toggle: Toggle,
}

impl Markdown {
    pub fn new(vim: Vim) -> Self {
        Self {
            vim,
            active_preview: None,
            toggle: Toggle::On,
        }
    }

    async fn update_toc(&self, bufnr: usize) -> Result<(), PluginError> {
        let file = self.vim.bufabspath(bufnr).await?;
        if let Some((start, end)) = find_toc_range(&file)? {
            let shiftwidth = self.vim.getbufvar("", "&shiftwidth").await?;
            // TODO: skip update if the new doc is the same as the old one.
            let new_toc = generate_toc(file, start + 1, shiftwidth)?;
            self.vim.deletebufline(bufnr, start + 1, end + 1).await?;
            self.vim.exec("append_and_write", json!([start, new_toc]))?;
        }
        Ok(())
    }
}

#[async_trait::async_trait]
impl ClapPlugin for Markdown {
    #[maple_derive::subscriptions]
    async fn handle_autocmd(&mut self, autocmd: AutocmdEvent) -> Result<(), PluginError> {
        use AutocmdEventType::{
            BufDelete, BufEnter, BufWritePost, CursorMoved, FileChangedShellPost, TextChangedI,
        };

        if self.toggle.is_off() {
            return Ok(());
        }

        if self.active_preview.is_none() {
            return Ok(());
        }

        let (event_type, params) = autocmd;
        let bufnr = params.parse_bufnr()?;

        match event_type {
            BufEnter => {
                if let Some(preview) = &mut self.active_preview {
                    // If we switched to a different markdown buffer, update the preview
                    if preview.bufnr != bufnr {
                        // Check if this is a markdown file
                        let filetype: String =
                            self.vim.getbufvar(&bufnr.to_string(), "&filetype").await?;
                        if filetype == "markdown" {
                            let path = self.vim.bufabspath(bufnr).await?;
                            tracing::debug!(
                                old_bufnr = preview.bufnr,
                                new_bufnr = bufnr,
                                "Switching preview to newly focused buffer"
                            );
                            preview
                                .msg_tx
                                .send_replace(Message::FileChanged(path, false));
                            // Update the tracked buffer number
                            preview.bufnr = bufnr;
                        }
                    }
                }
            }
            CursorMoved => {
                if let Some(preview) = &self.active_preview {
                    if preview.bufnr == bufnr {
                        let scroll_persent =
                            self.vim.line(".").await? * 100 / self.vim.line("$").await?;
                        preview.msg_tx.send_replace(Message::Scroll(scroll_persent));
                    }
                }
            }
            BufWritePost | FileChangedShellPost => {
                if let Some(preview) = &self.active_preview {
                    if preview.bufnr == bufnr {
                        let path = self.vim.bufabspath(bufnr).await?;
                        preview
                            .msg_tx
                            .send_replace(Message::FileChanged(path, false));
                    }
                }
            }
            TextChangedI => {
                if let Some(preview) = &self.active_preview {
                    if preview.bufnr == bufnr {
                        // TODO: incremental update?
                        let lines = self.vim.getbufline(bufnr, 1, "$").await?;
                        let markdown_content = lines.join("\n");
                        let (html, _line_map) = maple_markdown::to_html(&markdown_content)?;
                        preview.msg_tx.send_replace(Message::UpdateContent(html));
                    }
                }
            }
            BufDelete => {
                if let Some(preview) = &self.active_preview {
                    if preview.bufnr == bufnr {
                        // Remove preview and send shutdown signal
                        if let Some(preview) = self.active_preview.take() {
                            let _ = preview.shutdown_tx.send(());
                            tracing::debug!(
                                bufnr,
                                "Sent shutdown signal to markdown preview server"
                            );
                        }
                    }
                }
            }
            event => return Err(PluginError::UnhandledEvent(event)),
        }

        Ok(())
    }

    async fn handle_action(&mut self, action: PluginAction) -> Result<(), PluginError> {
        let PluginAction { method, params: _ } = action;
        match self.parse_action(method)? {
            MarkdownAction::GenerateToc => {
                let file = self.vim.current_buffer_path().await?;
                let curlnum = self.vim.line(".").await?;
                let shiftwidth = self.vim.getbufvar("", "&shiftwidth").await?;
                let mut toc = generate_toc(file, curlnum, shiftwidth)?;
                let prev_line = self.vim.curbufline(curlnum - 1).await?;
                if !prev_line.map(|line| line.is_empty()).unwrap_or(false) {
                    toc.push_front(Default::default());
                }
                self.vim
                    .exec("append_and_write", json!([curlnum - 1, toc]))?;
            }
            MarkdownAction::UpdateToc => {
                let bufnr = self.vim.bufnr("").await?;
                self.update_toc(bufnr).await?;
            }
            MarkdownAction::DeleteToc => {
                let file = self.vim.current_buffer_path().await?;
                let bufnr = self.vim.bufnr("").await?;
                if let Some((start, end)) = find_toc_range(file)? {
                    self.vim.deletebufline(bufnr, start + 1, end + 1).await?;
                }
            }
            MarkdownAction::PreviewInBrowser => {
                let bufnr = self.vim.bufnr("").await?;
                let path = self.vim.bufabspath(bufnr).await?;

                // If preview already exists, just update it with the new file
                if let Some(preview) = &mut self.active_preview {
                    tracing::debug!(
                        old_bufnr = preview.bufnr,
                        new_bufnr = bufnr,
                        port = preview.port,
                        "Reusing existing preview, switching to new buffer"
                    );
                    // Send file changed message with focus request
                    preview
                        .msg_tx
                        .send_replace(Message::FileChanged(path.clone(), true));
                    preview.bufnr = bufnr;

                    // Show notification in Vim to remind user to switch to browser
                    self.vim.exec(
                        "clap#plugin#markdown#on_preview_updated",
                        serde_json::json!({}),
                    )?;

                    return Ok(());
                }

                // No existing preview, create a new one
                tracing::info!(
                    bufnr,
                    path = %path,
                    "Creating new markdown preview"
                );

                // Initialize the channel with the file to preview
                let (msg_tx, msg_rx) =
                    tokio::sync::watch::channel(Message::FileChanged(path.clone(), false));

                let config_port = maple_config::config().plugin.markdown.preview_port;
                let addr = format!("127.0.0.1:{config_port}");
                let listener = tokio::net::TcpListener::bind(&addr).await?;
                // Get the actual port that was bound (important when config_port is 0)
                let port = listener.local_addr()?.port();

                tracing::info!(port, "Preview server will listen on port");

                // Create shutdown channel for graceful server shutdown
                let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();

                // Create disconnect notification channel
                let (disconnect_tx, disconnect_rx) = tokio::sync::oneshot::channel();

                let file_path = path.clone();

                tokio::spawn(async move {
                    if let Err(err) =
                        maple_markdown::open_preview_in_browser(maple_markdown::PreviewConfig {
                            listener,
                            msg_rx,
                            shutdown_rx,
                            file_path: Some(file_path),
                            disconnect_tx: Some(disconnect_tx),
                        })
                        .await
                    {
                        tracing::error!(?err, "Failed to open markdown preview");
                    }
                    tracing::debug!(bufnr, "markdown preview exited");
                });

                // Spawn task to handle browser disconnect notification
                let vim_for_disconnect = self.vim.clone();
                tokio::spawn(async move {
                    if disconnect_rx.await.is_ok() {
                        tracing::info!(bufnr, "Browser disconnected, notifying Vim");
                        let _ = vim_for_disconnect.exec(
                            "clap#plugin#markdown#on_browser_closed",
                            serde_json::json!({"bufnr": bufnr}),
                        );
                    }
                });

                // Store the new active preview
                self.active_preview = Some(ActivePreview {
                    bufnr,
                    port,
                    msg_tx,
                    shutdown_tx,
                });
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_markdown_toc() {
        let file = std::env::current_dir()
            .unwrap()
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("README.md");
        println!();
        for line in generate_toc(file, 0, 2).unwrap() {
            println!("{line}");
        }
    }
}
