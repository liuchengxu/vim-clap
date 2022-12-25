use crate::stdio_server::handler::{OnMoveHandler, PreviewTarget};
use crate::stdio_server::provider::{ClapProvider, ProviderContext, ProviderSource};
use crate::stdio_server::types::VimProgressor;
use crate::stdio_server::vim::Vim;
use anyhow::Result;
use filter::{FilterContext, ParallelSource};
use parking_lot::Mutex;
use printer::DisplayLines;
use serde_json::json;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use subprocess::Exec;
use types::MatchedItem;

#[derive(Debug)]
enum FilterSource {
    File(PathBuf),
    Command(String),
}

#[derive(Debug)]
struct FilterControl {
    stop_signal: Arc<AtomicBool>,
    join_handle: JoinHandle<()>,
}

impl FilterControl {
    fn kill(self) {
        self.stop_signal.store(true, Ordering::SeqCst);
        let _ = self.join_handle.join();
    }
}

/// Start the parallel filter in a new thread.
fn run(
    query: String,
    number: usize,
    filter_source: FilterSource,
    context: &ProviderContext,
    vim: Vim,
) -> FilterControl {
    let stop_signal = Arc::new(AtomicBool::new(false));

    let join_handle = {
        let icon = context.env.icon;
        let display_winwidth = context.env.display_winwidth;
        let cwd = context.cwd.clone();
        let matcher_builder = context.env.matcher_builder.clone();
        let stop_signal = stop_signal.clone();

        std::thread::spawn(move || {
            if let Err(e) = filter::par_dyn_run_inprocess(
                &query,
                FilterContext::new(icon, Some(number), Some(display_winwidth), matcher_builder),
                match filter_source {
                    FilterSource::File(path) => ParallelSource::File(path),
                    FilterSource::Command(command) => {
                        ParallelSource::Exec(Box::new(Exec::shell(command).cwd(cwd)))
                    }
                },
                VimProgressor::new(&vim, stop_signal.clone()),
                stop_signal,
            ) {
                tracing::error!(error = ?e, "Error occured when filtering the cache source");
            }
        })
    };

    FilterControl {
        stop_signal,
        join_handle,
    }
}

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
pub struct DefaultProvider {
    context: ProviderContext,
    current_results: Arc<Mutex<Vec<MatchedItem>>>,
    runtimepath: Option<String>,
    display_winheight: Option<usize>,
    maybe_filter_control: Option<FilterControl>,
    maybe_grep_control: Option<GrepControl>,
    last_filter_control_killed: Arc<AtomicBool>,
}

impl DefaultProvider {
    pub fn new(context: ProviderContext) -> Self {
        Self {
            context,
            current_results: Arc::new(Mutex::new(Vec::new())),
            runtimepath: None,
            display_winheight: None,
            maybe_filter_control: None,
            maybe_grep_control: None,
            last_filter_control_killed: Arc::new(AtomicBool::new(true)),
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

impl DefaultProvider {
    async fn generic_on_typed(&mut self) -> Result<()> {
        let query = self.vim().input_get().await?;

        let quick_response =
            if let ProviderSource::Small { ref items, .. } = *self.context.provider_source.read() {
                let matched_items = filter::par_filter_items(
                    items,
                    &self
                        .context
                        .env
                        .matcher_builder
                        .clone()
                        .build(query.clone().into()),
                );
                // Take the first 200 entries and add an icon to each of them.
                let DisplayLines {
                    lines,
                    indices,
                    truncated_map,
                    icon_added,
                } = printer::decorate_lines(
                    matched_items.iter().take(200).cloned().collect(),
                    self.context.env.display_winwidth,
                    self.context.env.icon,
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
            let new_query = self.vim().input_get().await?;
            if new_query == query {
                self.vim()
                    .exec("clap#state#process_filter_message", json!([msg, true]))?;
                let mut current_results = self.current_results.lock();
                *current_results = matched_items;
            }
            return Ok(());
        }

        let filter_source = match *self.context.provider_source.read() {
            ProviderSource::Small { .. } | ProviderSource::Unactionable => {
                tracing::debug!("par_dyn_run can not be used for ProviderSource::Small and ProviderSource::Unactionable.");
                return Ok(());
            }
            ProviderSource::CachedFile { ref path, .. }
            | ProviderSource::PlainFile { ref path, .. } => FilterSource::File(path.clone()),
            ProviderSource::Command(ref cmd) => FilterSource::Command(cmd.to_string()),
        };

        let display_winheight = match self.display_winheight {
            Some(winheight) => winheight,
            None => {
                let display_winheight = self
                    .vim()
                    .call("winheight", json!([self.context.env.display.winid]))
                    .await?;
                self.display_winheight.replace(display_winheight);
                display_winheight
            }
        };

        if !self.last_filter_control_killed.load(Ordering::SeqCst) {
            tracing::debug!(
                ?query,
                "============================== Still busy with killing the last filter control, return..."
            );
            return Ok(());
        }

        // Kill the last par_dyn_run job if exists.
        if let Some(control) = self.maybe_filter_control.take() {
            self.last_filter_control_killed
                .store(false, Ordering::SeqCst);

            let last_filter_control_killed = self.last_filter_control_killed.clone();
            tokio::task::spawn_blocking(move || {
                control.kill();
                last_filter_control_killed.store(true, Ordering::SeqCst);
            });
        }

        let new_control = run(
            query,
            display_winheight,
            filter_source,
            &self.context,
            self.vim().clone(),
        );

        self.maybe_filter_control.replace(new_control);

        Ok(())
    }

    async fn grep_on_typed(&mut self) -> Result<()> {
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
}

#[async_trait::async_trait]
impl ClapProvider for DefaultProvider {
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
        if self.vim().provider_id().await? == "grep" {
            self.grep_on_typed().await
        } else {
            self.generic_on_typed().await
        }
    }

    fn handle_terminate(&mut self, session_id: u64) {
        // Kill the last par_dyn_run job if exists.
        if let Some(control) = self.maybe_filter_control.take() {
            // NOTE: The kill operation can not block current task.
            tokio::task::spawn_blocking(move || control.kill());
        }
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
