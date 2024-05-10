use crate::stdio_server::input::{KeyEvent, KeyEventType};
use crate::stdio_server::provider::hooks::{CachedPreviewImpl, Preview, PreviewTarget};
use crate::stdio_server::provider::{
    ClapProvider, Context, Direction, ProviderError, ProviderResult as Result,
};
use crate::stdio_server::vim::preview_syntax;
use icon::{icon_or_default, FOLDER_ICON};
use printer::Printer;
use serde_json::json;
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::path::{Path, PathBuf, MAIN_SEPARATOR};
use std::sync::Arc;
use types::{ClapItem, MatchResult};

#[inline]
fn file_name(path: &Path) -> &str {
    path.file_name()
        .and_then(std::ffi::OsStr::to_str)
        .expect("Path terminates in `..`")
}

fn to_string_nicer(path: PathBuf, enable_icon: bool) -> String {
    if path.is_dir() {
        let dir_name = file_name(&path);
        if enable_icon {
            format!("{FOLDER_ICON} {dir_name}{MAIN_SEPARATOR}")
        } else {
            format!("{dir_name}{MAIN_SEPARATOR}")
        }
    } else if enable_icon {
        format!("{} {}", icon_or_default(&path), file_name(&path))
    } else {
        file_name(&path).to_string()
    }
}

pub fn read_dir_entries<P: AsRef<Path>>(
    dir: P,
    enable_icon: bool,
    max: Option<usize>,
) -> std::io::Result<Vec<String>> {
    let entries_iter =
        std::fs::read_dir(dir)?.map(|res| res.map(|x| to_string_nicer(x.path(), enable_icon)));

    let mut entries = if let Some(m) = max {
        entries_iter.take(m).collect::<std::io::Result<Vec<_>>>()?
    } else {
        entries_iter.collect::<std::io::Result<Vec<_>>>()?
    };

    entries.sort();

    Ok(entries)
}

#[derive(Debug)]
pub struct FilerItemWithoutIcon(pub String);

impl ClapItem for FilerItemWithoutIcon {
    fn raw_text(&self) -> &str {
        self.0.as_str()
    }
}

#[derive(Debug)]
pub struct FilerItem(pub String);

impl ClapItem for FilerItem {
    fn raw_text(&self) -> &str {
        self.0.as_str()
    }

    fn match_text(&self) -> &str {
        &self.0[4..]
    }

    fn match_result_callback(&self, match_result: MatchResult) -> MatchResult {
        let mut match_result = match_result;
        match_result.indices.iter_mut().for_each(|x| {
            *x += 4;
        });
        match_result
    }
}

#[derive(Debug)]
pub struct FilerProvider {
    current_dir: PathBuf,
    dir_entries: HashMap<PathBuf, Vec<Arc<dyn ClapItem>>>,
    current_lines: Vec<String>,
    printer: Printer,
    icon_enabled: bool,
    winwidth: usize,
}

impl FilerProvider {
    pub async fn new(ctx: &Context) -> Result<Self> {
        let current_dir = ctx.cwd.to_path_buf();
        // icon is handled inside the provider impl.
        let printer = Printer::new(ctx.env.display_winwidth, icon::Icon::Null);
        let icon_enabled = ctx.vim.get_var_bool("clap_enable_icon").await?;
        let winwidth = ctx.vim.winwidth(ctx.env.display.winid).await?;
        Ok(Self {
            current_dir,
            dir_entries: HashMap::new(),
            current_lines: Vec::new(),
            printer,
            winwidth,
            icon_enabled,
        })
    }

    // Strip the leading icon.
    async fn current_line(&self, ctx: &Context) -> Result<String> {
        let curline = ctx.vim.display_getcurline().await?;
        let curline = if self.icon_enabled {
            curline.chars().skip(2).collect()
        } else {
            curline
        };
        Ok(curline)
    }

    async fn on_tab(&mut self, ctx: &mut Context) -> Result<()> {
        let curline = self.current_line(ctx).await?;
        let target_dir = self.current_dir.join(curline);

        if target_dir.is_dir() {
            self.goto_dir(target_dir, ctx)?;
            self.preview_current_entry(ctx).await?;
        } else if target_dir.is_file() {
            let preview_target = PreviewTarget::StartOfFile(target_dir);
            self.update_preview(preview_target, ctx).await?;
        }

        Ok(())
    }

    async fn on_backspace(&mut self, ctx: &mut Context) -> Result<()> {
        let mut input: String = if ctx.env.is_nvim {
            ctx.vim.input_get().await?
        } else {
            ctx.vim
                .eval("g:__clap_popup_input_before_backspace_applied")
                .await?
        };

        if input.is_empty() {
            self.goto_parent(ctx)?;
            ctx.vim.exec(
                "clap#file_explorer#set_prompt",
                serde_json::json!([&self.current_dir, self.winwidth]),
            )?;
        } else {
            input.pop();
            ctx.vim.exec("input_set", [&input])?;
        }

        self.current_lines = self.on_query_change(&input, ctx)?;
        self.preview_current_entry(ctx).await
    }

    async fn on_carriage_return(&mut self, ctx: &Context) -> Result<()> {
        let curline = self.current_line(ctx).await?;
        let target_dir = self.current_dir.join(curline);

        if target_dir.is_dir() {
            self.goto_dir(target_dir, ctx)?;
        } else if target_dir.is_file() {
            ctx.vim.exec("execute", ["stopinsert"])?;
            ctx.vim.exec("clap#provider#filer#sink", [target_dir])?;
        } else {
            let input = ctx.vim.input_get().await?;
            let target_file = self.current_dir.join(input);
            ctx.vim
                .exec("clap#file_explorer#handle_special_entries", [target_file])?;
        }

        Ok(())
    }

    async fn preview_current_entry(&self, ctx: &mut Context) -> Result<()> {
        let curline = self.current_line(ctx).await?;
        let target_dir = self.current_dir.join(curline);
        let preview_target = if target_dir.is_dir() {
            PreviewTarget::Directory(target_dir)
        } else if target_dir.is_file() {
            PreviewTarget::StartOfFile(target_dir)
        } else {
            return Ok(());
        };

        self.update_preview(preview_target, ctx).await
    }

    fn on_query_change(&self, query: &str, ctx: &Context) -> Result<Vec<String>> {
        let current_items = self
            .dir_entries
            .get(&self.current_dir)
            .ok_or_else(|| ProviderError::Other("Directory entries not found".to_string()))?;

        let processed = current_items.len();

        if query.is_empty() {
            let mut display_lines = self.printer.to_display_lines(
                current_items
                    .iter()
                    .take(200)
                    .cloned()
                    .map(Into::into)
                    .collect(),
            );

            if self.icon_enabled {
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
                ..Default::default()
            };

            ctx.vim.exec("clap#picker#update", &update_info)?;

            return Ok(update_info.display_lines.lines);
        }

        let matcher = ctx.matcher_builder().build(query.into());
        let mut matched_items = filter::par_filter_items(current_items, &matcher);
        let matched = matched_items.len();

        matched_items.truncate(200);

        let mut display_lines = self.printer.to_display_lines(matched_items);

        if self.icon_enabled {
            display_lines.indices.iter_mut().for_each(|v| {
                v.iter_mut().for_each(|x| {
                    *x -= 2;
                })
            });
        }

        let update_info = printer::PickerUpdateInfo {
            matched,
            processed,
            display_lines,
            ..Default::default()
        };

        ctx.vim.exec("clap#picker#update", &update_info)?;

        Ok(update_info.display_lines.lines)
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
            Ok((preview_target, preview)) => {
                ctx.preview_manager.reset_scroll();
                ctx.update_picker_preview(preview)?;

                let maybe_syntax = preview_target.path().and_then(|path| {
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

                ctx.preview_manager.set_preview_target(preview_target);

                Ok(())
            }
            Err(err) => ctx
                .update_picker_preview(Preview::new(vec![err.to_string()]))
                .map_err(Into::into),
        }
    }

    fn goto_dir(&mut self, dir: PathBuf, ctx: &Context) -> Result<()> {
        self.current_dir.clone_from(&dir);
        self.load_dir(dir, ctx)?;
        ctx.vim.exec("input_set", [""])?;
        ctx.vim.exec(
            "clap#file_explorer#set_prompt",
            serde_json::json!([&self.current_dir, self.winwidth]),
        )?;
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
            let entries = match read_dir_entries(&self.current_dir, self.icon_enabled, None) {
                Ok(entries) => entries,
                Err(err) => {
                    ctx.vim
                        .exec("clap#provider#filer#handle_error", [err.to_string()])?;
                    return Ok(());
                }
            };

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

#[async_trait::async_trait]
impl ClapProvider for FilerProvider {
    async fn on_initialize(&mut self, ctx: &mut Context) -> Result<()> {
        let cwd = &ctx.cwd;

        let entries = match read_dir_entries(cwd, self.icon_enabled, None) {
            Ok(entries) => entries,
            Err(err) => {
                tracing::error!(?cwd, "Failed to read directory entries");
                ctx.vim
                    .exec("clap#provider#filer#handle_error", [err.to_string()])?;
                return Ok(());
            }
        };

        let response = json!({ "entries": &entries, "dir": cwd, "total": entries.len() });
        ctx.vim
            .exec("clap#file_explorer#handle_on_initialize", response)?;

        self.dir_entries.insert(
            cwd.to_path_buf(),
            entries
                .clone()
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
        self.current_lines = entries;

        Ok(())
    }

    async fn on_move(&mut self, ctx: &mut Context) -> Result<()> {
        if !ctx.env.preview_enabled {
            return Ok(());
        }
        self.preview_current_entry(ctx).await
    }

    async fn on_typed(&mut self, ctx: &mut Context) -> Result<()> {
        if self.current_lines.is_empty() {
            ctx.vim
                .bare_exec("clap#provider#filer#set_create_file_entry")?;
            return Ok(());
        }

        let query: String = ctx.vim.input_get().await?;
        let lines = self.on_query_change(&query, ctx)?;
        self.current_lines = lines;

        if self.current_lines.is_empty() {
            ctx.vim
                .bare_exec("clap#provider#filer#set_create_file_entry")?;
        }

        Ok(())
    }

    async fn on_key_event(&mut self, ctx: &mut Context, key_event: KeyEvent) -> Result<()> {
        let (key_event_type, _params) = key_event;
        match key_event_type {
            KeyEventType::Tab => self.on_tab(ctx).await,
            KeyEventType::Backspace => self.on_backspace(ctx).await,
            KeyEventType::CarriageReturn => self.on_carriage_return(ctx).await,
            KeyEventType::ShiftUp => ctx.scroll_preview(Direction::Up).await,
            KeyEventType::ShiftDown => ctx.scroll_preview(Direction::Down).await,
            KeyEventType::CtrlN => ctx.next_input().await,
            KeyEventType::CtrlP => ctx.prev_input().await,
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
