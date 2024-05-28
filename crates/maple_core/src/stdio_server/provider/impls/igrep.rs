use super::filer::{read_dir_entries, FilerItem, FilerItemWithoutIcon};
use crate::stdio_server::input::{KeyEvent, KeyEventType};
use crate::stdio_server::provider::hooks::{CachedPreviewImpl, Preview, PreviewTarget};
use crate::stdio_server::provider::{
    ClapProvider, Context, Direction, ProviderError, ProviderResult as Result, SearcherControl,
};
use crate::stdio_server::vim::preview_syntax;
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

#[derive(Debug)]
struct Grepper {
    searcher_control: Option<SearcherControl>,
}

impl Grepper {
    fn new() -> Self {
        Self {
            searcher_control: None,
        }
    }

    fn kill_last_searcher(&mut self) {
        if let Some(control) = self.searcher_control.take() {
            control.kill_in_background();
        }
    }

    fn grep(&mut self, query: String, path: PathBuf, ctx: &Context) {
        if let Some(control) = self.searcher_control.take() {
            control.kill_in_background();
        }

        let matcher = ctx
            .matcher_builder()
            .match_scope(MatchScope::Full) // Force using MatchScope::Full.
            .build(Query::from(&query));

        let new_control = {
            let stop_signal = Arc::new(AtomicBool::new(false));
            let vim = ctx.vim.clone();

            let mut search_context = ctx.search_context(stop_signal.clone());
            search_context.paths = vec![path];
            let join_handle = tokio::spawn(async move {
                let future = crate::searcher::grep::search(query, matcher, search_context);
                vim.search_with_spinner(future).await
            });

            SearcherControl {
                stop_signal,
                join_handle,
            }
        };

        self.searcher_control.replace(new_control);

        let _ = ctx
            .vim
            .setbufvar(ctx.env.display.bufnr, "&syntax", "clap_grep");
    }
}

#[derive(Debug)]
struct Explorer {
    printer: Printer,
    current_dir: PathBuf,
    dir_entries_cache: HashMap<PathBuf, Vec<Arc<dyn ClapItem>>>,
    current_lines: Vec<String>,
    icon_enabled: bool,
    winwidth: usize,
}

impl Explorer {
    async fn new(ctx: &Context) -> Result<Self> {
        let current_dir = ctx.cwd.to_path_buf();
        let printer = Printer::new(ctx.env.display_winwidth, icon::Icon::Null);
        let icon_enabled = ctx.vim.get_var_bool("clap_enable_icon").await?;
        let winwidth = ctx.vim.winwidth(ctx.env.display.winid).await?;
        Ok(Self {
            printer,
            current_dir,
            dir_entries_cache: HashMap::new(),
            current_lines: Vec::new(),
            icon_enabled,
            winwidth,
        })
    }

    async fn init(&mut self, ctx: &Context) -> Result<()> {
        let cwd = &ctx.cwd;

        let entries = match read_dir_entries(cwd, ctx.env.icon.enabled(), None) {
            Ok(entries) => entries,
            Err(err) => {
                tracing::error!(?cwd, "Failed to read directory entries");
                ctx.vim.exec("show_lines_in_preview", [err.to_string()])?;
                return Ok(());
            }
        };

        let query: String = ctx.vim.input_get().await?;
        if query.is_empty() {
            let response = json!({ "entries": &entries, "dir": cwd, "total": entries.len() });
            ctx.vim
                .exec("clap#file_explorer#handle_on_initialize", response)?;
            self.current_lines.clone_from(&entries);
        }

        self.dir_entries_cache.insert(
            cwd.to_path_buf(),
            entries
                .into_iter()
                .map(|line| Arc::new(FilerItem(line)) as Arc<dyn ClapItem>)
                .collect(),
        );

        ctx.vim
            .setbufvar(ctx.env.display.bufnr, "&syntax", "clap_filer")?;

        Ok(())
    }

    // Strip the leading filer icon.
    async fn current_line(&self, ctx: &Context) -> Result<String> {
        let curline = ctx.vim.display_getcurline().await?;
        let curline = if self.icon_enabled {
            curline.chars().skip(2).collect()
        } else {
            curline
        };
        Ok(curline)
    }

    async fn expand_dir_or_preview(&mut self, ctx: &mut Context) -> Result<()> {
        let curline = self.current_line(ctx).await?;
        let target_dir = self.current_dir.join(curline);

        if target_dir.is_dir() {
            self.goto_dir(target_dir, ctx)?;
            self.preview_current_line(ctx).await?;
        } else if target_dir.is_file() {
            let preview_target = PreviewTarget::StartOfFile(target_dir);
            self.update_preview_with_target(preview_target, ctx).await?;
        }
        Ok(())
    }

    async fn goto_parent(&mut self, ctx: &mut Context) -> Result<()> {
        self.load_parent(ctx)?;
        ctx.vim.exec(
            "clap#file_explorer#set_prompt",
            serde_json::json!([&self.current_dir, self.winwidth]),
        )?;
        self.current_lines = self.display_dir_entries(ctx)?;
        self.preview_current_line(ctx).await?;
        Ok(())
    }

    async fn apply_sink(&mut self, ctx: &Context) -> Result<()> {
        let curline = self.current_line(ctx).await?;
        let target_dir = self.current_dir.join(curline);
        if target_dir.is_dir() {
            self.goto_dir(target_dir, ctx)?;
        } else if target_dir.is_file() {
            ctx.vim.exec("execute", ["stopinsert"])?;
            ctx.vim.exec("clap#file_explorer#sink", [target_dir])?;
        } else {
            let input = ctx.vim.input_get().await?;
            let target_file = self.current_dir.join(input);
            ctx.vim
                .exec("clap#file_explorer#handle_special_entries", [target_file])?;
        }
        Ok(())
    }

    fn show_dir_entries(&mut self, ctx: &Context) -> Result<()> {
        self.current_lines = self.display_dir_entries(ctx)?;
        Ok(())
    }

    /// Display the file explorer.
    fn display_dir_entries(&self, ctx: &Context) -> Result<Vec<String>> {
        let current_items = self
            .dir_entries_cache
            .get(&self.current_dir)
            .ok_or_else(|| {
                ProviderError::Other(format!(
                    "Entries for {} not loaded",
                    self.current_dir.display()
                ))
            })?;

        let processed = current_items.len();

        let mut display_lines = self.printer.to_display_lines(
            current_items
                .iter()
                .take(200)
                .cloned()
                .map(Into::into)
                .collect(),
        );

        if ctx.env.icon.enabled() {
            display_lines.indices.iter_mut().for_each(|v| {
                v.iter_mut().for_each(|x| {
                    *x -= 2;
                })
            });
        }

        let update_info = printer::PickerUpdateInfo {
            matched: 0,
            processed,
            display_lines,
            display_syntax: Some("clap_filer".to_string()),
            ..Default::default()
        };

        ctx.vim.exec("clap#picker#update", &update_info)?;

        Ok(update_info.display_lines.lines)
    }

    async fn preview_current_line(&self, ctx: &mut Context) -> Result<()> {
        let curline = self.current_line(ctx).await?;
        let target_dir = self.current_dir.join(curline);
        let preview_target = if target_dir.is_dir() {
            PreviewTarget::Directory(target_dir)
        } else {
            PreviewTarget::StartOfFile(target_dir)
        };

        self.update_preview_with_target(preview_target, ctx).await
    }

    async fn update_preview_with_target(
        &self,
        preview_target: PreviewTarget,
        ctx: &mut Context,
    ) -> Result<()> {
        let preview_height = ctx.preview_height().await?;

        let preview_impl = CachedPreviewImpl {
            ctx,
            preview_height,
            preview_target,
            cache_line: None,
        };

        match preview_impl.get_preview().await {
            Ok((_preview_target, preview)) => {
                ctx.update_picker_preview(preview)?;

                let maybe_syntax = preview_impl.preview_target.path().and_then(|path| {
                    if path.is_dir() {
                        Some("clap_filer")
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
                ctx.update_picker_preview(Preview::new(vec![err.to_string()]))?;
            }
        }
        Ok(())
    }

    fn goto_dir(&mut self, dir: PathBuf, ctx: &Context) -> Result<()> {
        self.current_dir.clone_from(&dir);
        if let Err(err) = self.read_entries_if_not_in_cache(dir) {
            ctx.vim.exec("show_lines_in_preview", [err.to_string()])?;
        }
        ctx.vim.exec("input_set", [""])?;
        ctx.vim.exec(
            "clap#file_explorer#set_prompt",
            serde_json::json!([&self.current_dir, self.winwidth]),
        )?;
        self.current_lines = self.display_dir_entries(ctx)?;
        Ok(())
    }

    fn load_parent(&mut self, ctx: &Context) -> Result<()> {
        let parent_dir = match self.current_dir.parent() {
            Some(parent) => parent,
            None => return Ok(()),
        };
        self.current_dir = parent_dir.to_path_buf();
        if let Err(err) = self.read_entries_if_not_in_cache(self.current_dir.clone()) {
            ctx.vim.exec("show_lines_in_preview", [err.to_string()])?;
        }

        Ok(())
    }

    fn read_entries_if_not_in_cache(&mut self, target_dir: PathBuf) -> Result<()> {
        if let Entry::Vacant(v) = self.dir_entries_cache.entry(target_dir) {
            let entries = read_dir_entries(&self.current_dir, self.icon_enabled, None)?;

            v.insert(
                entries
                    .into_iter()
                    .map(|line| {
                        if self.icon_enabled {
                            Arc::new(FilerItem(line)) as Arc<dyn ClapItem>
                        } else {
                            Arc::new(FilerItemWithoutIcon(line)) as Arc<dyn ClapItem>
                        }
                    })
                    .collect(),
            );
        }

        Ok(())
    }
}

#[derive(Debug)]
enum Mode {
    FileExplorer,
    FileSearcher,
}

/// Grep in an interactive way.
#[derive(Debug)]
pub struct IgrepProvider {
    explorer: Explorer,
    grepper: Grepper,
    mode: Mode,
}

impl IgrepProvider {
    pub async fn new(ctx: &Context) -> Result<Self> {
        Ok(Self {
            explorer: Explorer::new(ctx).await?,
            grepper: Grepper::new(),
            mode: Mode::FileExplorer,
        })
    }

    async fn on_tab(&mut self, ctx: &mut Context) -> Result<()> {
        let input = ctx.vim.input_get().await?;
        if input.is_empty() {
            self.explorer.expand_dir_or_preview(ctx).await?;
        } else {
            ctx.vim.bare_exec("clap#selection#toggle")?;
        }

        Ok(())
    }

    async fn on_backspace(&mut self, ctx: &mut Context) -> Result<()> {
        self.grepper.kill_last_searcher();

        let mut input: String = if ctx.env.is_nvim {
            ctx.vim.input_get().await?
        } else {
            ctx.vim
                .eval("g:__clap_popup_input_before_backspace_applied")
                .await?
        };

        if input.is_empty() {
            self.explorer.goto_parent(ctx).await?;
        } else {
            input.pop();
            ctx.vim.exec("input_set", [&input])?;

            if input.is_empty() {
                self.explorer.show_dir_entries(ctx)?;
            } else {
                self.grepper
                    .grep(input, self.explorer.current_dir.clone(), ctx)
            }
        }

        Ok(())
    }

    async fn on_carriage_return(&mut self, ctx: &Context) -> Result<()> {
        match self.mode {
            Mode::FileExplorer => {
                self.explorer.apply_sink(ctx).await?;
            }
            Mode::FileSearcher => {
                let curline = ctx.vim.display_getcurline().await?;
                let grep_line = self.explorer.current_dir.join(curline);
                let (fpath, lnum, col, _line_content) = grep_line
                    .to_str()
                    .and_then(pattern::extract_grep_position)
                    .ok_or_else(|| {
                        ProviderError::Other(format!(
                            "Can not extract grep position: {}",
                            grep_line.display()
                        ))
                    })?;
                if !std::path::Path::new(fpath).is_file() {
                    ctx.vim.echo_info(format!("{fpath} is not a file"))?;
                    return Ok(());
                }
                ctx.vim.exec(
                    "clap#handler#sink_with",
                    json!(["clap#sink#open_file", fpath, lnum, col]),
                )?;
            }
        }

        Ok(())
    }

    async fn preview_grep_line(&self, ctx: &mut Context) -> Result<()> {
        let curline = ctx.vim.display_getcurline().await?;
        if let Some((fpath, lnum, _col, _cache_line)) = extract_grep_position(&curline) {
            let fpath = fpath.strip_prefix("./").unwrap_or(fpath);
            let path = self.explorer.current_dir.join(fpath);

            let preview_target = PreviewTarget::location_in_file(path, lnum);

            ctx.update_preview(Some(preview_target)).await?;
        }
        Ok(())
    }
}

#[async_trait::async_trait]
impl ClapProvider for IgrepProvider {
    async fn on_initialize(&mut self, ctx: &mut Context) -> Result<()> {
        self.explorer.init(ctx).await
    }

    async fn on_move(&mut self, ctx: &mut Context) -> Result<()> {
        if !ctx.env.preview_enabled {
            return Ok(());
        }
        let query: String = ctx.vim.input_get().await?;
        if query.is_empty() {
            self.explorer.preview_current_line(ctx).await
        } else {
            self.preview_grep_line(ctx).await
        }
    }

    async fn on_typed(&mut self, ctx: &mut Context) -> Result<()> {
        let query: String = ctx.vim.input_get().await?;

        if query.is_empty() {
            self.mode = Mode::FileExplorer;
            self.explorer.show_dir_entries(ctx)?;
        } else {
            self.mode = Mode::FileSearcher;
            self.grepper
                .grep(query, self.explorer.current_dir.clone(), ctx);
        }

        Ok(())
    }

    async fn on_key_event(&mut self, ctx: &mut Context, key_event: KeyEvent) -> Result<()> {
        let (key_event_type, _params) = key_event;
        match key_event_type {
            KeyEventType::CtrlN => ctx.next_input().await,
            KeyEventType::CtrlP => ctx.prev_input().await,
            KeyEventType::ShiftUp => ctx.scroll_preview(Direction::Up).await,
            KeyEventType::ShiftDown => ctx.scroll_preview(Direction::Down).await,
            KeyEventType::Tab => self.on_tab(ctx).await,
            KeyEventType::Backspace => self.on_backspace(ctx).await,
            KeyEventType::CarriageReturn => self.on_carriage_return(ctx).await,
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
