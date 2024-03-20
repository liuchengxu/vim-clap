use crate::searcher::file::BlinesItem;
use crate::stdio_server::provider::hooks::PreviewTarget;
use crate::stdio_server::provider::{
    BaseArgs, ClapProvider, Context, ProviderResult as Result, SearcherControl,
};
use crate::stdio_server::vim::VimResult;
use matcher::{Bonus, MatchScope};
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use types::{ClapItem, Query};

#[derive(Debug)]
enum BufferSource {
    /// Buffer is modified, we need to fetch the latest content via VIM api.
    ModifiedBuffer(Vec<Arc<dyn ClapItem>>),
    /// Buffer is unmodified, we can simply read it from the local file.
    LocalFile(PathBuf),
}

#[derive(Debug)]
pub struct BlinesProvider {
    args: BaseArgs,
    searcher_control: Option<SearcherControl>,
    preview_file: PathBuf,
    source: BufferSource,
}

impl BlinesProvider {
    pub async fn new(ctx: &Context) -> VimResult<Self> {
        let args = ctx.parse_provider_args().await?;

        let bufnr = ctx.vim.eval::<usize>("g:clap.start.bufnr").await?;

        let (source, preview_file) = if ctx.vim.bufmodified(bufnr).await? {
            let lines = ctx.vim.getbufline(bufnr, 1, "$").await?;
            let file_content = lines.join("\n");

            let mut items = lines
                .into_iter()
                .enumerate()
                .map(|(index, line)| {
                    Arc::new(BlinesItem {
                        raw: line,
                        line_number: index + 1,
                    })
                })
                .collect::<Vec<_>>();

            items.sort_by_key(|i| i.line_number);

            let items = items
                .into_iter()
                .map(|item| item as Arc<dyn ClapItem>)
                .collect::<Vec<_>>();

            // Write the modified buffer to a tmp file and preview it later.
            let bufname = ctx
                .env
                .start_buffer_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("blines_preview");
            let tmp_file = crate::datastore::generate_cache_file_path(bufname)?;
            std::fs::File::create(&tmp_file)?.write_all(file_content.as_bytes())?;

            (BufferSource::ModifiedBuffer(items), tmp_file)
        } else {
            let path = ctx.env.start_buffer_path.clone();
            (BufferSource::LocalFile(path.clone()), path)
        };

        // TODO: refactor the preview_on_empty function so that this dirty
        // workaround can be removed.
        ctx.set_provider_source(crate::stdio_server::provider::ProviderSource::File {
            total: 16, // Unused, does not matter.
            path: preview_file.clone(),
        });

        Ok(Self {
            args,
            searcher_control: None,
            preview_file,
            source,
        })
    }

    fn process_query_on_local_file(&mut self, query: String, ctx: &Context, source_file: PathBuf) {
        if let Some(control) = self.searcher_control.take() {
            control.kill_in_background();
        }

        let matcher_builder = ctx.matcher_builder().match_scope(MatchScope::Full);

        let matcher = if let Some(extension) = source_file.extension().and_then(|s| s.to_str()) {
            matcher_builder
                .bonuses(vec![Bonus::Language(extension.into())])
                .build(Query::from(&query))
        } else {
            matcher_builder.build(Query::from(&query))
        };

        let new_control = {
            let stop_signal = Arc::new(AtomicBool::new(false));

            let join_handle = {
                let search_context = ctx.search_context(stop_signal.clone());

                tokio::spawn(async move {
                    crate::searcher::file::search(query, source_file, matcher, search_context)
                        .await;
                })
            };

            SearcherControl {
                stop_signal,
                join_handle,
            }
        };

        self.searcher_control.replace(new_control);
    }

    fn process_query(&mut self, query: String, ctx: &Context) -> Result<()> {
        match &self.source {
            BufferSource::ModifiedBuffer(items) => {
                let matched_items = filter::par_filter_items(items, &ctx.matcher(&query));
                let printer = printer::Printer::new(ctx.env.display_winwidth, ctx.env.icon);
                let display_lines =
                    printer.to_display_lines(matched_items.iter().take(200).cloned().collect());

                let update_info = printer::PickerUpdateInfo {
                    matched: matched_items.len(),
                    processed: items.len(),
                    display_lines,
                    ..Default::default()
                };

                ctx.vim.exec("clap#picker#update", update_info)?;
            }
            BufferSource::LocalFile(file) => {
                self.process_query_on_local_file(query, ctx, file.clone());
            }
        }

        Ok(())
    }
}

#[async_trait::async_trait]
impl ClapProvider for BlinesProvider {
    async fn on_initialize(&mut self, ctx: &mut Context) -> Result<()> {
        if self.args.query.is_none() {
            ctx.update_on_empty_query().await?;
        } else {
            ctx.handle_base_args(&self.args).await?;
        }
        Ok(())
    }

    async fn on_move(&mut self, ctx: &mut Context) -> Result<()> {
        if !ctx.env.preview_enabled {
            return Ok(());
        }
        ctx.preview_manager.reset_scroll();
        let curline = ctx.vim.display_getcurline().await?;
        let Some(line_number) = pattern::extract_blines_lnum(&curline) else {
            return Ok(());
        };
        let preview_target = PreviewTarget::LineInFile {
            path: self.preview_file.clone(),
            line_number,
        };
        ctx.update_preview(Some(preview_target)).await
    }

    async fn on_typed(&mut self, ctx: &mut Context) -> Result<()> {
        let query = ctx.vim.input_get().await?;
        if query.is_empty() {
            ctx.update_on_empty_query().await?;
        } else {
            self.process_query(query, ctx)?;
        }
        Ok(())
    }

    fn on_terminate(&mut self, ctx: &mut Context, session_id: u64) {
        if let Some(control) = self.searcher_control.take() {
            control.kill_in_background();
        }
        if let BufferSource::ModifiedBuffer(_) = self.source {
            let _ = std::fs::remove_file(&self.preview_file);
        }
        ctx.signify_terminated(session_id);
    }
}
