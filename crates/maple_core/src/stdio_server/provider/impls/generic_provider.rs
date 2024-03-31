use crate::stdio_server::provider::hooks::{initialize_provider, CachedPreviewImpl, PreviewTarget};
use crate::stdio_server::provider::{
    BaseArgs, ClapProvider, Context, ProviderError, ProviderResult as Result, ProviderSource,
};
use crate::stdio_server::SearchProgressor;
use filter::{FilterContext, ParallelSource};
use parking_lot::Mutex;
use printer::Printer;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use subprocess::Exec;
use types::MatchedItem;

#[derive(Debug)]
enum DataSource {
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
fn start_filter_parallel(
    query: String,
    number: usize,
    data_source: DataSource,
    ctx: &Context,
) -> FilterControl {
    let stop_signal = Arc::new(AtomicBool::new(false));

    let join_handle = {
        let filter_context = FilterContext::new(
            ctx.env.icon,
            Some(number),
            Some(ctx.env.display_winwidth),
            ctx.matcher_builder(),
        );

        let cwd = ctx.cwd.clone();
        let vim = ctx.vim.clone();
        let stop_signal = stop_signal.clone();

        std::thread::spawn(move || {
            if let Err(e) = filter::par_dyn_run_inprocess(
                &query,
                filter_context,
                match data_source {
                    DataSource::File(path) => ParallelSource::File(path),
                    DataSource::Command(command) => {
                        ParallelSource::Exec(Box::new(Exec::shell(command).cwd(cwd)))
                    }
                },
                SearchProgressor::new(vim, stop_signal.clone()),
                stop_signal,
            ) {
                tracing::error!(error = ?e, "Error occurred when filtering the cache source");
            }
        })
    };

    FilterControl {
        stop_signal,
        join_handle,
    }
}

/// Generic provider impl.
#[derive(Debug)]
pub struct GenericProvider {
    args: BaseArgs,
    runtimepath: Option<String>,
    maybe_filter_control: Option<FilterControl>,
    current_results: Arc<Mutex<Vec<MatchedItem>>>,
    last_filter_control_killed: Arc<AtomicBool>,
}

impl GenericProvider {
    pub async fn new(ctx: &Context) -> Result<Self> {
        let args = ctx.parse_provider_args().await?;
        Ok(Self {
            args,
            runtimepath: None,
            maybe_filter_control: None,
            current_results: Arc::new(Mutex::new(Vec::new())),
            last_filter_control_killed: Arc::new(AtomicBool::new(true)),
        })
    }

    /// `lnum` is 1-based.
    #[allow(unused)]
    fn line_at(&self, lnum: usize) -> Option<String> {
        self.current_results
            .lock()
            .get(lnum - 1)
            .map(|r| r.item.output_text().to_string())
    }

    async fn nontypical_preview_target(
        &mut self,
        curline: &str,
        ctx: &Context,
    ) -> Result<Option<PreviewTarget>> {
        let maybe_preview_kind = match ctx.provider_id() {
            "help_tags" => {
                let runtimepath = match &self.runtimepath {
                    Some(rtp) => rtp.clone(),
                    None => {
                        let rtp: String = ctx.vim.eval("&runtimepath").await?;
                        self.runtimepath.replace(rtp.clone());
                        rtp
                    }
                };
                let items = curline.split('\t').collect::<Vec<_>>();
                if items.len() < 2 {
                    return Err(ProviderError::Other(format!(
                        "Couldn't extract subject and doc_filename from {curline}"
                    )));
                }
                Some(PreviewTarget::HelpTags {
                    subject: items[0].trim().to_string(),
                    doc_filename: items[1].trim().to_string(),
                    runtimepath,
                })
            }
            "buffers" => {
                let res: [String; 2] = ctx
                    .vim
                    .bare_call("clap#provider#buffers#preview_target")
                    .await?;
                let mut iter = res.into_iter();
                let path = iter.next().expect("Element must exist").into();
                let line_number = iter.next().expect("Element must exist").parse::<usize>()?;
                Some(PreviewTarget::location_in_file(path, line_number))
            }
            _ => None,
        };

        Ok(maybe_preview_kind)
    }
}

#[async_trait::async_trait]
impl ClapProvider for GenericProvider {
    async fn on_initialize(&mut self, ctx: &mut Context) -> Result<()> {
        let init_display = self.args.query.is_none();
        // Always attempt to initialize the source
        initialize_provider(ctx, init_display).await?;
        ctx.handle_base_args(&self.args).await
    }

    async fn on_move(&mut self, ctx: &mut Context) -> Result<()> {
        if !ctx.env.preview_enabled {
            return Ok(());
        }

        let lnum = ctx.vim.display_getcurlnum().await?;

        let curline = ctx.vim.display_getcurline().await?;

        if curline.is_empty() {
            tracing::debug!("Skipping preview as curline is empty");
            return Ok(());
        }

        let preview_height = ctx.preview_height().await?;
        let preview_impl =
            if let Some(preview_target) = self.nontypical_preview_target(&curline, ctx).await? {
                CachedPreviewImpl {
                    ctx,
                    preview_height,
                    preview_target,
                    cache_line: None,
                }
            } else {
                CachedPreviewImpl::new(curline, preview_height, ctx)?
            };

        let (preview_target, preview) = preview_impl.get_preview().await?;

        // Ensure the preview result is not out-dated.
        let curlnum = ctx.vim.display_getcurlnum().await?;
        if curlnum == lnum {
            ctx.update_picker_preview(preview)?;
        }

        ctx.preview_manager.set_preview_target(preview_target);

        Ok(())
    }

    async fn on_typed(&mut self, ctx: &mut Context) -> Result<()> {
        let query = ctx.vim.input_get().await?;

        // Handle the empty separately, otherwise the order of items may be altered
        // due to the filtering later, which is inconsistent with `on_initialize` behaviour.
        if query.is_empty() {
            ctx.update_on_empty_query().await?;
            return Ok(());
        }

        let small_list_response =
            if let ProviderSource::Small { ref items, .. } = *ctx.provider_source.read() {
                let matched_items = filter::par_filter_items(items, &ctx.matcher(&query));
                let printer = Printer::new(ctx.env.display_winwidth, ctx.env.icon);
                // Take the first 200 entries and add an icon to each of them.
                let display_lines =
                    printer.to_display_lines(matched_items.iter().take(200).cloned().collect());
                let update_info = printer::PickerUpdateInfo {
                    matched: matched_items.len(),
                    processed: items.len(),
                    display_lines,
                    ..Default::default()
                };
                Some((update_info, matched_items))
            } else {
                None
            };

        if let Some((update_info, matched_items)) = small_list_response {
            let new_query = ctx.vim.input_get().await?;
            if new_query == query {
                ctx.vim.exec("clap#picker#update", update_info)?;
                let mut current_results = self.current_results.lock();
                *current_results = matched_items;
            }
            return Ok(());
        }

        let data_source = match *ctx.provider_source.read() {
            ProviderSource::Small { .. } => unreachable!("Handled above; qed"),
            ProviderSource::Initializing => {
                ctx.vim
                    .echo_warn("Can not process query: source initialization is in progress")?;
                ctx.initializing_prompt_echoed.store(true, Ordering::SeqCst);
                return Ok(());
            }
            ProviderSource::Uninitialized => {
                ctx.vim
                    .echo_warn("Can not process query: source uninitialized")?;
                return Ok(());
            }
            ProviderSource::InitializationFailed(ref msg) => {
                ctx.vim.echo_warn(format!("InitializationFailed: {msg}"))?;
                return Ok(());
            }
            ProviderSource::CachedFile { ref path, .. } | ProviderSource::File { ref path, .. } => {
                DataSource::File(path.clone())
            }
            ProviderSource::Command(ref cmd) => DataSource::Command(cmd.to_string()),
        };

        if !self.last_filter_control_killed.load(Ordering::SeqCst) {
            tracing::debug!(
                ?query,
                "Still busy with killing the last filter control, return..."
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

        let display_winheight = ctx.env.display_winheight;
        let new_control = start_filter_parallel(query, display_winheight, data_source, ctx);

        self.maybe_filter_control.replace(new_control);

        Ok(())
    }

    fn on_terminate(&mut self, ctx: &mut Context, session_id: u64) {
        if let Some(control) = self.maybe_filter_control.take() {
            // NOTE: The kill operation can not block current task.
            tokio::task::spawn_blocking(move || control.kill());
        }
        ctx.signify_terminated(session_id);
    }
}
