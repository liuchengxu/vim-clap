use super::filer::{read_dir_entries, FilerItem};
use super::Direction;
use crate::stdio_server::handler::{CachedPreviewImpl, Preview, PreviewTarget};
use crate::stdio_server::input::KeyEvent;
use crate::stdio_server::provider::{ClapProvider, Context, SearcherControl};
use crate::stdio_server::vim::preview_syntax;
use anyhow::Result;
use matcher::MatchScope;
use pattern::extract_grep_position;
use printer::Printer;
use serde_json::json;
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use types::{ClapItem, Query};

/// Grep in an interactive way.
#[derive(Debug)]
pub struct IgrepProvider {
    printer: Printer,
    current_dir: PathBuf,
    dir_entries: HashMap<PathBuf, Vec<Arc<dyn ClapItem>>>,
    current_lines: Vec<String>,
    searcher_control: Option<SearcherControl>,
}

impl IgrepProvider {
    pub fn new(ctx: &Context) -> Self {
        let current_dir = ctx.cwd.to_path_buf();
        let printer = Printer::new(ctx.env.display_winwidth, icon::Icon::Null);
        Self {
            printer,
            current_dir,
            dir_entries: HashMap::new(),
            current_lines: Vec::new(),
            searcher_control: None,
        }
    }

    // Without the icon.
    async fn current_line(&self, ctx: &Context) -> Result<String> {
        let curline = ctx.vim.display_getcurline().await?;
        tracing::debug!("============= [current_line] curline: {curline:?}");
        let curline = if ctx.vim.get_var_bool("clap_enable_icon").await? {
            curline.chars().skip(2).collect()
        } else {
            curline
        };
        Ok(curline)
    }

    async fn on_tab(&mut self, ctx: &mut Context) -> Result<()> {
        let input = ctx.vim.input_get().await?;
        tracing::debug!("============= [on_tab] input: {input:?}");
        if input.is_empty() {
            let curline = self.current_line(ctx).await?;
            let target_dir = self.current_dir.join(curline);

            if target_dir.is_dir() {
                self.goto_dir(target_dir, ctx)?;
                self.preview_current_entry(ctx).await?;
            } else if target_dir.is_file() {
                let preview_target = PreviewTarget::File(target_dir);
                self.update_preview(preview_target, ctx).await?;
            }
        } else {
            ctx.vim.bare_exec("clap#selection#toggle")?;
        }

        Ok(())
    }

    async fn on_backspace(&mut self, ctx: &mut Context) -> Result<()> {
        if let Some(control) = self.searcher_control.take() {
            tokio::task::spawn_blocking(move || {
                control.kill();
            });
        }

        let mut input = ctx.vim.input_get().await?;

        if input.is_empty() {
            self.goto_parent(ctx)?;
            ctx.vim
                .exec("clap#provider#igrep#set_prompt", [&self.current_dir])?;
            self.current_lines = self.on_query_change(&input, ctx)?;
            self.preview_current_entry(ctx).await?;
        } else {
            input.pop();
            ctx.vim.exec("input_set", [&input])?;

            if input.is_empty() {
                let lines = self.on_query_change("", ctx)?;
                self.current_lines = lines;
            } else {
                self.start_grep(input, ctx);
            }
        }

        Ok(())
    }

    async fn on_carriage_return(&mut self, ctx: &Context) -> Result<()> {
        let curline = self.current_line(ctx).await?;
        let target_dir = self.current_dir.join(curline);

        if target_dir.is_dir() {
            self.goto_dir(target_dir, ctx)?;
        } else if target_dir.is_file() {
            ctx.vim.exec("execute", ["stopinsert"])?;
            ctx.vim.exec("clap#provider#igrep#sink", [target_dir])?;
        } else {
            let input = ctx.vim.input_get().await?;
            let target_file = self.current_dir.join(input);
            ctx.vim
                .exec("clap#provider#igrep#handle_special_entries", [target_file])?;
        }

        Ok(())
    }

    async fn preview_current_entry(&self, ctx: &mut Context) -> Result<()> {
        let curline = self.current_line(ctx).await?;
        tracing::debug!("====== [preview_current_entry] curline: {curline:?}");
        let target_dir = self.current_dir.join(curline);
        let preview_target = if target_dir.is_dir() {
            PreviewTarget::Directory(target_dir)
        } else {
            PreviewTarget::File(target_dir)
        };

        self.update_preview(preview_target, ctx).await
    }

    fn on_query_change(&self, query: &str, ctx: &Context) -> Result<Vec<String>> {
        let current_items = self
            .dir_entries
            .get(&self.current_dir)
            .ok_or_else(|| anyhow::anyhow!("Directory entries not found"))?;

        let processed = current_items.len();

        if query.is_empty() {
            let printer::DisplayLines {
                lines,
                mut indices,
                truncated_map: _,
                icon_added,
            } = self.printer.to_display_lines(
                current_items
                    .iter()
                    .take(200)
                    .cloned()
                    .map(Into::into)
                    .collect(),
            );

            if ctx.env.icon.enabled() {
                indices.iter_mut().for_each(|v| {
                    v.iter_mut().for_each(|x| {
                        *x -= 2;
                    })
                });
            }

            let result = json!({
                "lines": &lines, "indices": indices, "matched": 0, "processed": processed, "icon_added": icon_added,
            });

            ctx.vim
                .exec("clap#state#process_filter_message", json!([result, true]))?;

            return Ok(lines);
        }

        let matcher = ctx.matcher_builder().build(query.into());
        let mut matched_items = filter::par_filter_items(current_items, &matcher);
        let matched = matched_items.len();

        matched_items.truncate(200);

        let printer::DisplayLines {
            lines,
            mut indices,
            truncated_map,
            icon_added,
        } = self.printer.to_display_lines(matched_items);

        if ctx.env.icon.enabled() {
            indices.iter_mut().for_each(|v| {
                v.iter_mut().for_each(|x| {
                    *x -= 2;
                })
            });
        }

        let result = if truncated_map.is_empty() {
            json!({ "lines": &lines, "indices": indices, "matched": matched, "processed": processed, "icon_added": icon_added })
        } else {
            json!({ "lines": &lines, "indices": indices, "matched": matched, "processed": processed, "icon_added": icon_added, "truncated_map": truncated_map })
        };

        ctx.vim
            .exec("clap#state#process_filter_message", json!([result, true]))?;

        Ok(lines)
    }

    async fn update_preview(&self, preview_target: PreviewTarget, ctx: &mut Context) -> Result<()> {
        let preview_height = ctx.preview_height().await?;

        let preview_impl = CachedPreviewImpl {
            ctx,
            preview_height,
            preview_target,
            cache_line: None,
        };

        match preview_impl.get_preview().await {
            Ok((_preview_target, preview)) => {
                ctx.render_preview(preview)?;

                let maybe_syntax = preview_impl.preview_target.path().and_then(|path| {
                    if path.is_dir() {
                        Some("clap_grep")
                    } else if path.is_file() {
                        preview_syntax(path)
                    } else {
                        None
                    }
                });

                if let Some(syntax) = maybe_syntax {
                    ctx.vim.set_preview_syntax(syntax)?;
                }
            }
            Err(err) => {
                ctx.render_preview(Preview::new(vec![err.to_string()]))?;
            }
        }
        Ok(())
    }

    fn goto_dir(&mut self, dir: PathBuf, ctx: &Context) -> Result<()> {
        self.current_dir = dir.clone();
        self.load_dir(dir, ctx)?;
        ctx.vim.exec("input_set", [""])?;
        ctx.vim
            .exec("clap#provider#igrep#set_prompt", [&self.current_dir])?;
        let lines = self.on_query_change("", ctx)?;
        self.current_lines = lines;
        Ok(())
    }

    fn goto_parent(&mut self, ctx: &Context) -> Result<()> {
        let parent_dir = match self.current_dir.parent() {
            Some(parent) => parent,
            None => return Ok(()),
        };
        self.current_dir = parent_dir.to_path_buf();
        self.load_dir(self.current_dir.clone(), ctx)
    }

    fn load_dir(&mut self, target_dir: PathBuf, ctx: &Context) -> Result<()> {
        if let Entry::Vacant(v) = self.dir_entries.entry(target_dir) {
            let entries = match read_dir_entries(&self.current_dir, ctx.env.icon.enabled(), None) {
                Ok(entries) => entries,
                Err(err) => {
                    ctx.vim
                        .exec("clap#provider#igrep#handle_error", [err.to_string()])?;
                    return Ok(());
                }
            };

            v.insert(
                entries
                    .into_iter()
                    .map(|line| Arc::new(FilerItem(line)) as Arc<dyn ClapItem>)
                    .collect(),
            );
        }

        Ok(())
    }

    fn start_grep(&mut self, query: String, ctx: &Context) {
        if let Some(control) = self.searcher_control.take() {
            tokio::task::spawn_blocking(move || {
                control.kill();
            });
        }

        let matcher = ctx
            .matcher_builder()
            .match_scope(MatchScope::Full) // Force using MatchScope::Full.
            .build(Query::from(&query));

        let new_control = {
            let stop_signal = Arc::new(AtomicBool::new(false));

            let mut search_context = ctx.search_context(stop_signal.clone());
            search_context.paths = vec![self.current_dir.clone()];
            tracing::debug!(
                "====================== [start_grep] search_context: {search_context:?}"
            );
            let join_handle = tokio::spawn(async move {
                crate::searcher::grep::search(query, matcher, search_context).await
            });

            SearcherControl {
                stop_signal,
                join_handle,
            }
        };

        self.searcher_control.replace(new_control);
    }
}

#[async_trait::async_trait]
impl ClapProvider for IgrepProvider {
    async fn on_initialize(&mut self, ctx: &mut Context) -> Result<()> {
        let cwd = &ctx.cwd;

        let entries = match read_dir_entries(cwd, ctx.env.icon.enabled(), None) {
            Ok(entries) => entries,
            Err(err) => {
                tracing::error!(?cwd, "Failed to read directory entries");
                ctx.vim
                    .exec("clap#provider#igrep#handle_error", [err.to_string()])?;
                return Ok(());
            }
        };

        let query: String = ctx.vim.input_get().await?;
        if query.is_empty() {
            let response = json!({ "entries": &entries, "dir": cwd, "total": entries.len() });
            ctx.vim
                .exec("clap#provider#igrep#handle_on_initialize", response)?;
            self.current_lines = entries.clone();
        }

        self.dir_entries.insert(
            cwd.to_path_buf(),
            entries
                .into_iter()
                .map(|line| Arc::new(FilerItem(line)) as Arc<dyn ClapItem>)
                .collect(),
        );

        Ok(())
    }

    async fn on_move(&mut self, ctx: &mut Context) -> Result<()> {
        if !ctx.env.preview_enabled {
            return Ok(());
        }
        let query: String = ctx.vim.input_get().await?;
        if query.is_empty() {
            self.preview_current_entry(ctx).await?;
        } else {
            let curline = ctx.vim.display_getcurline().await?;
            if let Some((fpath, lnum, _col, _cache_line)) = extract_grep_position(&curline) {
                let fpath = fpath.strip_prefix("./").unwrap_or(fpath);
                let path = self.current_dir.join(fpath);

                let preview_target = PreviewTarget::LineInFile {
                    path,
                    line_number: lnum,
                };

                ctx.update_preview(Some(preview_target)).await?;
            }
        }
        Ok(())
    }

    async fn on_typed(&mut self, ctx: &mut Context) -> Result<()> {
        let query: String = ctx.vim.input_get().await?;

        if query.is_empty() {
            let lines = self.on_query_change(&query, ctx)?;
            self.current_lines = lines;
        } else {
            self.start_grep(query, ctx);
        }

        Ok(())
    }

    async fn on_key_event(&mut self, ctx: &mut Context, key_event: KeyEvent) -> Result<()> {
        match key_event {
            KeyEvent::Tab => self.on_tab(ctx).await,
            KeyEvent::Backspace => self.on_backspace(ctx).await,
            KeyEvent::CarriageReturn => self.on_carriage_return(ctx).await,
            KeyEvent::ShiftUp => ctx.scroll_preview(Direction::Up).await,
            KeyEvent::ShiftDown => ctx.scroll_preview(Direction::Down).await,
            KeyEvent::CtrlN => ctx.next_input().await,
            KeyEvent::CtrlP => ctx.previous_input().await,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dir() {
        // /home/xlc/.vim/plugged/vim-clap/crates/stdio_server
        let entries = read_dir_entries(
            std::env::current_dir()
                .unwrap()
                .into_os_string()
                .into_string()
                .unwrap(),
            false,
            None,
        )
        .unwrap();

        assert_eq!(entries, vec!["Cargo.toml", "src/"]);
    }
}
