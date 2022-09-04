mod on_create;
mod on_move;
mod providers;

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use anyhow::Result;
use parking_lot::Mutex;
use serde_json::json;
use std::thread::JoinHandle;
use subprocess::Exec;

use filter::{FilterContext, ParSource};
use printer::DisplayLines;
use types::MatchedItem;

use crate::stdio_server::provider::{ClapProvider, ProviderSource};
use crate::stdio_server::session::SessionContext;
use crate::stdio_server::types::VimProgressor;

pub use self::on_create::initialize_provider_source;
pub use self::on_move::{OnMoveHandler, PreviewKind};
pub use self::providers::{dumb_jump, filer, recent_files};

use super::vim::Vim;

#[derive(Debug)]
enum FilterSource {
    CachedFile(PathBuf),
    Command(String),
}

#[derive(Debug)]
struct FilterControl {
    stop_signal: Arc<AtomicBool>,
    thread_filter: JoinHandle<()>,
}

impl FilterControl {
    fn kill(self) {
        tracing::debug!("Killing");
        self.stop_signal.store(true, Ordering::Relaxed);
        let _ = self.thread_filter.join();
    }
}

fn run(
    query: String,
    filter_source: FilterSource,
    context: &SessionContext,
    vim: Vim,
) -> FilterControl {
    let stop_signal = Arc::new(AtomicBool::new(false));

    let thread_filter = {
        let icon = context.icon;
        let display_winwidth = context.display_winwidth;
        let matcher = context.matcher.clone();
        let stop_signal = stop_signal.clone();

        std::thread::spawn(move || {
            if let Err(e) = filter::par_dyn_run(
                &query,
                FilterContext::new(icon, Some(40), Some(display_winwidth), matcher),
                match filter_source {
                    FilterSource::CachedFile(path) => ParSource::File(path),
                    FilterSource::Command(command) => {
                        ParSource::Exec(Box::new(Exec::shell(command)))
                    }
                },
                VimProgressor::new(&vim),
                Some(stop_signal),
            ) {
                tracing::error!(error = ?e, "Error occured when filtering the cache source");
            }
        })
    };

    FilterControl {
        stop_signal,
        thread_filter,
    }
}

#[derive(Debug)]
pub struct DefaultProvider {
    vim: Vim,
    context: SessionContext,
    current_results: Arc<Mutex<Vec<MatchedItem>>>,
    runtimepath: Option<String>,
    maybe_filter_control: Option<FilterControl>,
}

impl DefaultProvider {
    pub fn new(vim: Vim, context: SessionContext) -> Self {
        Self {
            vim,
            context,
            current_results: Arc::new(Mutex::new(Vec::new())),
            runtimepath: None,
            maybe_filter_control: None,
        }
    }

    /// `lnum` is 1-based.
    #[allow(unused)]
    fn line_at(&self, lnum: usize) -> Option<String> {
        self.current_results
            .lock()
            .get((lnum - 1) as usize)
            .map(|r| r.item.output_text().to_string())
    }
}

#[async_trait::async_trait]
impl ClapProvider for DefaultProvider {
    fn vim(&self) -> &Vim {
        &self.vim
    }

    fn session_context(&self) -> &SessionContext {
        &self.context
    }

    async fn on_move(&mut self) -> Result<()> {
        let lnum = self.vim.display_getcurlnum().await?;

        let curline = self.vim.display_getcurline().await?;

        if curline.is_empty() {
            tracing::debug!("Skipping preview as curline is empty");
            return Ok(());
        }

        let preview_size = self
            .vim
            .preview_size(&self.context.provider_id, self.context.preview.winid)
            .await?;

        let on_move_handler = if self.context.provider_id.as_str() == "help_tags" {
            let runtimepath = match &self.runtimepath {
                Some(rtp) => rtp.clone(),
                None => {
                    let rtp: String = self.vim.eval("&runtimepath").await?;
                    self.runtimepath.replace(rtp.clone());
                    rtp
                }
            };
            let items = curline.split('\t').collect::<Vec<_>>();
            if items.len() < 2 {
                return Err(anyhow::anyhow!(
                    "Couldn't extract subject and doc_filename from the line"
                ));
            }
            let preview_kind = PreviewKind::HelpTags {
                subject: items[0].trim().to_string(),
                doc_filename: items[1].trim().to_string(),
                runtimepath,
            };
            OnMoveHandler {
                size: preview_size,
                context: &self.context,
                preview_kind,
                cache_line: None,
            }
        } else {
            OnMoveHandler::create(curline, preview_size, &self.context)?
        };

        // TODO: Cache the preview.
        let preview = on_move_handler.get_preview().await?;

        // Ensure the preview result is not out-dated.
        let curlnum = self.vim.display_getcurlnum().await?;
        if curlnum == lnum {
            self.vim
                .exec("clap#state#process_preview_result", preview)?;
        }

        Ok(())
    }

    async fn on_typed(&mut self) -> Result<()> {
        let query = self.vim.input_get().await?;

        let quick_response =
            if let ProviderSource::Small { ref items, .. } = *self.context.provider_source.read() {
                let matched_items = filter::par_filter_items(&query, items, &self.context.matcher);
                // Take the first 200 entries and add an icon to each of them.
                let DisplayLines {
                    lines,
                    indices,
                    truncated_map,
                    icon_added,
                } = printer::decorate_lines(
                    matched_items.iter().take(200).cloned().collect(),
                    self.context.display_winwidth as usize,
                    self.context.icon,
                );
                let msg = json!({
                    "total": matched_items.len(),
                    "lines": lines,
                    "indices": indices,
                    "icon_added": icon_added,
                    "truncated_map": truncated_map,
                });
                Some((msg, matched_items))
            } else {
                None
            };

        if let Some((msg, matched_items)) = quick_response {
            let new_query = self.vim.input_get().await?;
            if new_query == query {
                self.vim()
                    .exec("clap#state#process_filter_message", json!([msg, true]))?;
                let mut current_results = self.current_results.lock();
                *current_results = matched_items;
            }
            return Ok(());
        }

        let filter_source = match *self.context.provider_source.read() {
            ProviderSource::Small { .. } | ProviderSource::Unknown => {
                tracing::debug!("par_dyn_run can not be used for ProviderSource::Small and ProviderSource::Unknown.");
                return Ok(());
            }
            ProviderSource::CachedFile { ref path, .. } => FilterSource::CachedFile(path.clone()),
            ProviderSource::Command(ref cmd) => FilterSource::Command(cmd.to_string()),
        };

        // Kill the last par_dyn_run job if exists.
        if let Some(control) = self.maybe_filter_control.take() {
            control.kill();
        }

        let new_control = run(query, filter_source, &self.context, self.vim.clone());

        self.maybe_filter_control.replace(new_control);

        Ok(())
    }
}
