use crate::searcher::blines::BlinesItem;
use crate::stdio_server::provider::{
    BaseArgs, ClapProvider, Context, ProviderResult as Result, ProviderSource, SearcherControl,
};
use crate::stdio_server::vim::VimResult;
use matcher::{Bonus, MatchScope};
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use types::{ClapItem, Query};

#[derive(Debug)]
enum BufferSource {
    Items(Vec<Arc<dyn ClapItem>>),
    LocalFile(PathBuf),
}

#[derive(Debug)]
pub struct BlinesProvider {
    args: BaseArgs,
    searcher_control: Option<SearcherControl>,
    source: BufferSource,
}

impl BlinesProvider {
    pub async fn new(ctx: &Context) -> VimResult<Self> {
        let args = ctx.parse_provider_args().await?;

        let bufnr = ctx.vim.eval::<usize>("g:clap.start.bufnr").await?;

        let source = if ctx.vim.bufmodified(bufnr).await? {
            let lines = ctx.vim.getbufline(bufnr, 1, "$").await?;
            let items = lines
                .into_iter()
                .enumerate()
                .map(|(index, line)| {
                    Arc::new(BlinesItem {
                        raw: line,
                        line_number: index + 1,
                    }) as Arc<dyn types::ClapItem>
                })
                .collect::<Vec<_>>();

            // Initialize the provider source to reuse the on_move impl.
            ctx.set_provider_source(ProviderSource::Small {
                total: items.len(),
                items: items.clone(),
            });

            BufferSource::Items(items)
        } else {
            let path = ctx.env.start_buffer_path.clone();
            let total = utils::line_count(&path)?;

            ctx.set_provider_source(ProviderSource::File {
                total,
                path: path.clone(),
            });

            BufferSource::LocalFile(path)
        };

        Ok(Self {
            args,
            searcher_control: None,
            source,
        })
    }

    fn process_query_on_local_file(&mut self, query: String, ctx: &Context, source_file: PathBuf) {
        if let Some(control) = self.searcher_control.take() {
            tokio::task::spawn_blocking(move || {
                control.kill();
            });
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
                    crate::searcher::blines::search(query, source_file, matcher, search_context)
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
}

#[async_trait::async_trait]
impl ClapProvider for BlinesProvider {
    async fn on_initialize(&mut self, ctx: &mut Context) -> Result<()> {
        if self.args.query.is_none() {
            // No longer need to invoke initialize_provider as we did it in new().
        } else {
            ctx.handle_base_args(&self.args).await?;
        }
        Ok(())
    }

    async fn on_typed(&mut self, ctx: &mut Context) -> Result<()> {
        let query = ctx.vim.input_get().await?;

        match &self.source {
            BufferSource::Items(items) => {
                let matched_items = filter::par_filter_items(items, &ctx.matcher(&query));
                let printer = printer::Printer::new(ctx.env.display_winwidth, ctx.env.icon);
                let printer::DisplayLines {
                    lines,
                    indices,
                    truncated_map,
                    icon_added,
                } = printer.to_display_lines(matched_items.iter().take(200).cloned().collect());

                let msg = serde_json::json!({
                    "total": matched_items.len(),
                    "lines": lines,
                    "indices": indices,
                    "icon_added": icon_added,
                    "truncated_map": truncated_map,
                });

                ctx.vim.exec(
                    "clap#state#process_filter_message",
                    serde_json::json!([msg, true]),
                )?;
            }
            BufferSource::LocalFile(file) => {
                self.process_query_on_local_file(query, ctx, file.clone());
            }
        }

        Ok(())
    }

    fn on_terminate(&mut self, ctx: &mut Context, session_id: u64) {
        if let Some(control) = self.searcher_control.take() {
            // NOTE: The kill operation can not block current task.
            tokio::task::spawn_blocking(move || control.kill());
        }
        ctx.signify_terminated(session_id);
    }
}
