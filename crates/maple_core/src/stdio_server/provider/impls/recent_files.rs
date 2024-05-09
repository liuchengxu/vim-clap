use crate::datastore::RECENT_FILES_IN_MEMORY;
use crate::stdio_server::provider::hooks::CachedPreviewImpl;
use crate::stdio_server::provider::{
    BaseArgs, ClapProvider, Context, ProviderError, ProviderResult as Result,
};
use parking_lot::Mutex;
use paths::AbsPathBuf;
use printer::Printer;
use serde_json::json;
use std::sync::Arc;
use types::{ClapItem, MatchedItem, RankCalculator, Score};

#[derive(Debug, Clone)]
pub struct RecentFilesProvider {
    args: BaseArgs,
    printer: Printer,
    lines: Arc<Mutex<Vec<MatchedItem>>>,
}

impl RecentFilesProvider {
    pub async fn new(ctx: &Context) -> Result<Self> {
        let args = ctx.parse_provider_args().await?;
        let icon = if ctx.env.icon.enabled() {
            icon::Icon::Enabled(icon::IconKind::File)
        } else {
            icon::Icon::Null
        };
        let printer = Printer::new(ctx.env.display_winwidth, icon);
        Ok(Self {
            args,
            printer,
            lines: Default::default(),
        })
    }

    fn process_query(
        self,
        cwd: AbsPathBuf,
        query: String,
        preview_size: Option<usize>,
        lnum: usize,
    ) -> Result<printer::PickerUpdateInfo> {
        let cwd = cwd.to_string();

        let mut recent_files = RECENT_FILES_IN_MEMORY.lock();
        let ranked = if query.is_empty() {
            // Sort the initial list according to the cwd.
            //
            // This changes the order of existing recent file entries.
            recent_files.sort_by_cwd(&cwd);

            let mut cwd = cwd.clone();
            cwd.push(std::path::MAIN_SEPARATOR);

            let rank_calculator = RankCalculator::default();

            recent_files
                .entries
                .iter()
                .map(|entry| {
                    let item: Arc<dyn ClapItem> = Arc::new(entry.fpath.clone());
                    // frecent_score will not be larger than i32::MAX.
                    let score = entry.frecent_score as Score;
                    let rank = rank_calculator.calculate_rank(score, 0, 0, item.raw_text().len());
                    let mut matched_item = MatchedItem::new(item, rank, Default::default());
                    matched_item
                        .output_text
                        .replace(entry.fpath.replacen(&cwd, "", 1));
                    matched_item
                })
                .collect::<Vec<_>>()
        } else {
            recent_files.filter_on_query(&query, cwd.clone())
        };

        let processed = recent_files.len();
        let matched = ranked.len();

        drop(recent_files);

        // process the new preview
        let preview = match (preview_size, ranked.get(lnum - 1)) {
            (Some(size), Some(new_entry)) => {
                let new_curline = new_entry.display_text().to_string();
                if let Ok((lines, fname)) =
                    crate::previewer::preview_file(new_curline, size, self.printer.line_width)
                {
                    Some(json!({ "lines": lines, "fname": fname }))
                } else {
                    None
                }
            }
            _ => None,
        };

        let printer::DisplayLines {
            lines,
            indices,
            truncated_map,
            icon_added,
        } = self
            .printer
            .to_display_lines(ranked.iter().take(200).cloned().collect());

        let mut cwd = cwd;
        cwd.push(std::path::MAIN_SEPARATOR);

        let lines = lines
            .into_iter()
            .map(|abs_path| abs_path.replacen(&cwd, "", 1))
            .collect::<Vec<_>>();

        // The indices are empty on the empty query.
        let indices = indices
            .into_iter()
            .filter(|i| !i.is_empty())
            .collect::<Vec<_>>();

        let update_info = printer::PickerUpdateInfo {
            matched,
            processed,
            display_lines: printer::DisplayLines {
                lines,
                indices,
                truncated_map,
                icon_added,
            },
            display_syntax: None,
            preview,
        };

        let mut lines = self.lines.lock();
        *lines = ranked;

        Ok(update_info)
    }
}

#[async_trait::async_trait]
impl ClapProvider for RecentFilesProvider {
    async fn on_initialize(&mut self, ctx: &mut Context) -> Result<()> {
        if self.args.query.is_none() {
            let preview_size = if ctx.env.preview_enabled {
                Some(ctx.preview_size().await?)
            } else {
                None
            };

            let update_info =
                self.clone()
                    .process_query(ctx.cwd.clone(), "".into(), preview_size, 1)?;

            ctx.vim.exec("clap#picker#update", update_info)?;
        } else {
            ctx.handle_base_args(&self.args).await?;
        }

        Ok(())
    }

    async fn on_move(&mut self, ctx: &mut Context) -> Result<()> {
        let lnum = ctx.vim.display_getcurlnum().await?;

        let maybe_curline = self
            .lines
            .lock()
            .get(lnum - 1)
            .map(|r| r.item.raw_text().to_string());

        if let Some(curline) = maybe_curline {
            let preview_height = ctx.preview_height().await?;
            let mut ctx = ctx.clone();
            tokio::spawn(async move {
                let (preview_target, preview) =
                    CachedPreviewImpl::new(curline, preview_height, &ctx)?
                        .get_preview()
                        .await?;
                ctx.preview_manager.reset_scroll();
                ctx.update_picker_preview(preview)?;
                ctx.preview_manager.set_preview_target(preview_target);
                Ok::<(), ProviderError>(())
            });
        }

        Ok(())
    }

    async fn on_typed(&mut self, ctx: &mut Context) -> Result<()> {
        let query = ctx.vim.input_get().await?;

        let update_info = tokio::task::spawn_blocking({
            let query = query.clone();
            let recent_files = self.clone();

            let cwd = ctx.cwd.clone();
            let preview_size = if ctx.env.preview_enabled {
                Some(ctx.preview_size().await?)
            } else {
                None
            };
            let lnum = ctx.vim.display_getcurlnum().await?;

            move || recent_files.process_query(cwd, query, preview_size, lnum)
        })
        .await??;

        let current_query = ctx.vim.input_get().await?;
        if current_query == query {
            ctx.vim.exec("clap#picker#update", update_info)?;
        }

        Ok(())
    }
}
