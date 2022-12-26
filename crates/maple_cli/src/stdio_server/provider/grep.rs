use crate::stdio_server::handler::{OnMoveHandler, PreviewTarget};
use crate::stdio_server::provider::{ClapProvider, ProviderContext};
use crate::stdio_server::types::VimProgressor;
use crate::stdio_server::vim::Vim;
use anyhow::Result;
use parking_lot::Mutex;
use serde_json::json;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use types::MatchedItem;

#[derive(Debug)]
struct GrepControl {
    stop_signal: Arc<AtomicBool>,
    join_handle: tokio::task::JoinHandle<()>,
}

impl GrepControl {
    fn kill(self) {
        self.stop_signal.store(true, Ordering::SeqCst);
        self.join_handle.abort();
    }
}

fn run_grep(query: String, number: usize, context: &ProviderContext, vim: Vim) -> GrepControl {
    let stop_signal = Arc::new(AtomicBool::new(false));

    let join_handle = {
        let icon = context.env.icon;
        let display_winwidth = context.env.display_winwidth;
        let cwd = context.cwd.clone();
        let matcher_builder = context.env.matcher_builder.clone();
        let stop_signal = stop_signal.clone();

        tokio::spawn(async move {
            let progressor = VimProgressor::new(&vim, stop_signal.clone());
            crate::searcher::search(
                cwd.into(),
                // Process against the line directly.
                matcher_builder
                    .match_scope(matcher::MatchScope::Full)
                    .build(query.into()),
                stop_signal,
                number,
                icon,
                display_winwidth,
                progressor,
            )
            .await;
        })
    };

    GrepControl {
        stop_signal,
        join_handle,
    }
}

#[derive(Debug)]
pub struct GrepProvider {
    context: ProviderContext,
    current_results: Arc<Mutex<Vec<MatchedItem>>>,
    runtimepath: Option<String>,
    maybe_grep_control: Option<GrepControl>,
}

impl GrepProvider {
    pub fn new(context: ProviderContext) -> Self {
        Self {
            context,
            current_results: Arc::new(Mutex::new(Vec::new())),
            runtimepath: None,
            maybe_grep_control: None,
        }
    }

    #[inline]
    fn vim(&self) -> &Vim {
        &self.context.vim
    }

    /// `lnum` is 1-based.
    #[allow(unused)]
    fn line_at(&self, lnum: usize) -> Option<String> {
        self.current_results
            .lock()
            .get(lnum - 1)
            .map(|r| r.item.output_text().to_string())
    }

    async fn nontypical_preview_target(&mut self, curline: &str) -> Result<Option<PreviewTarget>> {
        let maybe_preview_kind = match self.context.provider_id() {
            "help_tags" => {
                let runtimepath = match &self.runtimepath {
                    Some(rtp) => rtp.clone(),
                    None => {
                        let rtp: String = self.vim().eval("&runtimepath").await?;
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
                Some(PreviewTarget::HelpTags {
                    subject: items[0].trim().to_string(),
                    doc_filename: items[1].trim().to_string(),
                    runtimepath,
                })
            }
            "buffers" => {
                let res: Vec<String> = self
                    .vim()
                    .call("clap#provider#buffers#preview_target", json!([]))
                    .await?;
                if res.len() != 2 {
                    return Err(anyhow::anyhow!(
                        "Can not retrieve the buffers preview target"
                    ));
                }
                let line_number = res[1].parse::<usize>()?;
                let path = res
                    .into_iter()
                    .next()
                    .expect("Not empty as just checked; qed")
                    .into();
                Some(PreviewTarget::LineInFile { path, line_number })
            }
            _ => None,
        };

        Ok(maybe_preview_kind)
    }
}

#[async_trait::async_trait]
impl ClapProvider for GrepProvider {
    fn context(&self) -> &ProviderContext {
        &self.context
    }

    async fn on_move(&mut self) -> Result<()> {
        let lnum = self.vim().display_getcurlnum().await?;

        let curline = self.vim().display_getcurline().await?;

        if curline.is_empty() {
            tracing::debug!("Skipping preview as curline is empty");
            return Ok(());
        }

        let preview_height = self.context.preview_height().await?;
        let on_move_handler =
            if let Some(preview_target) = self.nontypical_preview_target(&curline).await? {
                OnMoveHandler {
                    preview_height,
                    context: &self.context,
                    preview_target,
                    cache_line: None,
                }
            } else {
                OnMoveHandler::create(curline, preview_height, &self.context)?
            };

        let preview = on_move_handler.get_preview().await?;

        // Ensure the preview result is not out-dated.
        let curlnum = self.vim().display_getcurlnum().await?;
        if curlnum == lnum {
            self.vim().render_preview(preview)?;
        }

        Ok(())
    }

    async fn on_typed(&mut self) -> Result<()> {
        let query = self.vim().input_get().await?;

        if let Some(control) = self.maybe_grep_control.take() {
            tokio::task::spawn_blocking(move || {
                control.kill();
            });
        }

        let new_control = run_grep(query, 100, &self.context, self.vim().clone());

        self.maybe_grep_control.replace(new_control);

        Ok(())
    }

    fn handle_terminate(&mut self, session_id: u64) {
        if let Some(control) = self.maybe_grep_control.take() {
            // NOTE: The kill operation can not block current task.
            tokio::task::spawn_blocking(move || control.kill());
        }
        self.context.terminated.store(true, Ordering::SeqCst);
        tracing::debug!(
            session_id,
            provider_id = %self.context.provider_id(),
            "Session terminated",
        );
    }
}
