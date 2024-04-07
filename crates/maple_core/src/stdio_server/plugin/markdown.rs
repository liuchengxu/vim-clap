#![allow(clippy::enum_variant_names)]

use crate::stdio_server::input::{AutocmdEvent, AutocmdEventType, PluginAction};
use crate::stdio_server::plugin::{ClapPlugin, PluginError, Toggle};
use crate::stdio_server::vim::Vim;
use maple_markdown::toc::{find_toc_range, generate_toc};
use maple_markdown::Message;
use serde_json::json;
use std::collections::HashMap;

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
    bufs: HashMap<usize, tokio::sync::watch::Sender<Message>>,
    toggle: Toggle,
}

impl Markdown {
    pub fn new(vim: Vim) -> Self {
        Self {
            vim,
            bufs: HashMap::new(),
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
        use AutocmdEventType::{BufDelete, BufWritePost, CursorMoved, TextChangedI};

        if self.toggle.is_off() {
            return Ok(());
        }

        if self.bufs.is_empty() {
            return Ok(());
        }

        let (event_type, params) = autocmd;
        let bufnr = params.parse_bufnr()?;

        match event_type {
            CursorMoved => {
                let scroll_persent = self.vim.line(".").await? * 100 / self.vim.line("$").await?;
                if let Some(msg_tx) = self.bufs.get(&bufnr) {
                    msg_tx.send_replace(Message::Scroll(scroll_persent));
                }
            }
            BufWritePost => {
                for (bufnr, msg_tx) in self.bufs.iter() {
                    let path = self.vim.bufabspath(bufnr).await?;
                    msg_tx.send_replace(Message::FileChanged(path));
                }
            }
            TextChangedI => {
                // TODO: incremental update?
                let lines = self.vim.getbufline(bufnr, 1, "$").await?;
                let markdown_content = lines.join("\n");
                let html =
                    markdown::to_html_with_options(&markdown_content, &markdown::Options::gfm())
                        .map_err(PluginError::Other)?;
                if let Some(msg_tx) = self.bufs.get(&bufnr) {
                    msg_tx.send_replace(Message::UpdateContent(html));
                }
            }
            BufDelete => {
                if let Some(msg_tx) = self.bufs.remove(&bufnr) {
                    // Drop the markdown worker message sender to exit the markdown preview task.
                    drop(msg_tx);
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
                let (msg_tx, msg_rx) =
                    tokio::sync::watch::channel(Message::UpdateContent(String::new()));

                let port = maple_config::config().plugin.markdown.preview_port;
                let addr = format!("127.0.0.1:{port}");
                let listener = tokio::net::TcpListener::bind(&addr).await?;

                let bufnr = self.vim.bufnr("").await?;

                tokio::spawn(async move {
                    if let Err(err) = maple_markdown::open_preview(listener, msg_rx).await {
                        tracing::error!(?err, "Failed to open markdown preview in browser");
                    }
                    tracing::debug!(bufnr, "markdown preview exited");
                });

                let path = self.vim.bufabspath(bufnr).await?;
                msg_tx.send_replace(Message::FileChanged(path));

                self.bufs.insert(bufnr, msg_tx);
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
