use crate::stdio_server::handler::{Preview, PreviewImpl, PreviewTarget};
use crate::stdio_server::input::KeyEvent;
use crate::stdio_server::provider::{ClapProvider, Context};
use crate::stdio_server::vim::preview_syntax;
use crate::utils::build_abs_path;
use anyhow::Result;
use icon::prepend_filer_icon;
use serde_json::json;
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::path::{Path, PathBuf, MAIN_SEPARATOR};
use std::sync::Arc;
use types::{ClapItem, MatchResult};

/// Display the inner path in a nicer way.
struct DisplayPath<P> {
    inner: P,
    enable_icon: bool,
}

impl<P: AsRef<Path>> DisplayPath<P> {
    fn new(inner: P, enable_icon: bool) -> Self {
        Self { inner, enable_icon }
    }

    #[inline]
    fn as_file_name_unsafe(&self) -> &str {
        self.inner
            .as_ref()
            .file_name()
            .and_then(std::ffi::OsStr::to_str)
            .expect("Path terminates in `..`")
    }
}

impl<P: AsRef<Path>> std::fmt::Display for DisplayPath<P> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let mut write_with_icon = |path: &str| {
            if self.enable_icon {
                write!(f, "{}", prepend_filer_icon(self.inner.as_ref(), path))
            } else {
                write!(f, "{path}")
            }
        };

        if self.inner.as_ref().is_dir() {
            let path = format!("{}{}", self.as_file_name_unsafe(), MAIN_SEPARATOR);
            write_with_icon(&path)
        } else {
            write_with_icon(self.as_file_name_unsafe())
        }
    }
}

#[allow(unused)]
fn goto_parent(cur_dir: String) {
    // Root directory.
    if Path::new(&cur_dir).parent().is_none() {
        // noop
        return;
    }

    let parent_dir = match Path::new(&cur_dir).parent() {
        Some(dir) => dir,
        None => return,
    };

    let _new_cur_dir = if parent_dir.parent().is_none() {
        parent_dir.to_string_lossy().to_string()
    } else {
        format!("{}{MAIN_SEPARATOR}", parent_dir.display())
    };

    if let Some(last_char) = cur_dir.chars().last() {
        if last_char == MAIN_SEPARATOR {}
    }
}

pub fn read_dir_entries<P: AsRef<Path>>(
    dir: P,
    enable_icon: bool,
    max: Option<usize>,
) -> std::io::Result<Vec<String>> {
    let entries_iter = std::fs::read_dir(dir)?
        .map(|res| res.map(|x| DisplayPath::new(x.path(), enable_icon).to_string()));

    let mut entries = if let Some(m) = max {
        entries_iter.take(m).collect::<std::io::Result<Vec<_>>>()?
    } else {
        entries_iter.collect::<std::io::Result<Vec<_>>>()?
    };

    entries.sort();

    Ok(entries)
}

#[derive(Debug)]
struct FilerItem(String);

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
}

impl FilerProvider {
    pub fn new(current_dir: PathBuf) -> Self {
        Self {
            current_dir,
            dir_entries: HashMap::new(),
            current_lines: Vec::new(),
        }
    }

    // Without the icon.
    async fn current_line(&self, ctx: &Context) -> Result<String> {
        let curline = ctx.vim.display_getcurline().await?;
        let curline = if ctx.vim.get_var_bool("clap_enable_icon").await? {
            curline.chars().skip(2).collect()
        } else {
            curline
        };
        Ok(curline)
    }

    async fn on_tab(&mut self, ctx: &mut Context) -> Result<()> {
        let curline = self.current_line(ctx).await?;
        let target_dir = self.current_dir.clone().join(curline);

        let preview_target = if target_dir.is_dir() {
            self.reset_to(target_dir, ctx)?;
            let curline = self.current_line(ctx).await?;
            let path: PathBuf = curline.into();
            if path.is_dir() {
                PreviewTarget::Directory(path)
            } else {
                PreviewTarget::File(path)
            }
        } else if target_dir.is_file() {
            PreviewTarget::File(target_dir.clone())
        } else {
            return Ok(());
        };
        let preview_height = ctx.preview_height().await?;

        self.update_preview(preview_target, preview_height, ctx)
            .await?;

        Ok(())
    }

    async fn on_backspace(&mut self, ctx: &mut Context) -> Result<()> {
        let mut input = ctx.vim.input_get().await?;

        if input.is_empty() {
            self.load_parent(ctx)?;
            ctx.vim
                .exec("clap#provider#filer#set_prompt", [&self.current_dir])?;
        } else {
            input.pop();
            ctx.vim.exec("input_set", [&input])?;
        }

        let lines = self.on_query_change(&input, ctx)?;
        self.current_lines = lines;

        let curline = self.current_line(ctx).await?;
        let target_dir = self.current_dir.clone().join(curline);
        let preview_target = if target_dir.is_dir() {
            PreviewTarget::Directory(target_dir)
        } else {
            PreviewTarget::File(target_dir)
        };
        let preview_height = ctx.preview_height().await?;

        self.update_preview(preview_target, preview_height, ctx)
            .await?;

        Ok(())
    }

    async fn on_carriage_return(&mut self, ctx: &Context) -> Result<()> {
        let curline = self.current_line(ctx).await?;
        let target_dir = self.current_dir.clone().join(curline);

        if target_dir.is_dir() {
            self.reset_to(target_dir, ctx)?;
            return Ok(());
        } else if target_dir.is_file() {
            ctx.vim.exec("execute", ["stopinsert"])?;
            ctx.vim.exec("clap#provider#filer#sink", [target_dir])?;
            return Ok(());
        }

        let input = ctx.vim.input_get().await?;
        let target_file = self.current_dir.clone().join(input);

        ctx.vim
            .call("clap#provider#filer#handle_special_entries", [target_file])
            .await?;

        Ok(())
    }

    fn on_query_change(&self, query: &str, ctx: &Context) -> Result<Vec<String>> {
        let current_items = self
            .dir_entries
            .get(&self.current_dir)
            .ok_or_else(|| anyhow::anyhow!("Directory entries not found"))?;

        let matched_items =
            filter::par_filter_items(current_items, &ctx.matcher_builder().build(query.into()));
        let total = matched_items.len();

        let printer::DisplayLines {
            lines,
            indices,
            truncated_map,
            icon_added,
        } = printer::decorate_lines(
            matched_items.iter().take(200).cloned().collect(),
            ctx.env.display_winwidth,
            icon::Icon::Null, // icon is handled inside the provider impl.
        );

        let result = if truncated_map.is_empty() {
            json!({ "lines": &lines, "indices": indices, "total": total, "icon_added": icon_added })
        } else {
            json!({ "lines": &lines, "indices": indices, "total": total, "icon_added": icon_added, "truncated_map": truncated_map })
        };

        ctx.vim
            .exec("clap#state#process_filter_message", json!([result, true]))?;

        Ok(lines)
    }

    fn reset_to(&mut self, dir: PathBuf, ctx: &Context) -> Result<()> {
        self.current_dir = dir.clone();
        self.load_dir(dir, ctx)?;
        ctx.vim.exec("input_set", [""])?;
        ctx.vim
            .exec("clap#provider#filer#set_prompt", [&self.current_dir])?;
        let lines = self.on_query_change("", ctx)?;
        self.current_lines = lines;
        Ok(())
    }

    async fn update_preview(
        &self,
        preview_target: PreviewTarget,
        preview_height: usize,
        ctx: &Context,
    ) -> Result<()> {
        let preview_impl = PreviewImpl {
            ctx,
            preview_height,
            preview_target,
            cache_line: None,
        };

        let maybe_syntax = preview_impl.preview_target.path().and_then(|path| {
            if path.is_dir() {
                Some("clap_filer")
            } else if path.is_file() {
                preview_syntax(path)
            } else {
                None
            }
        });

        match preview_impl.get_preview().await {
            Ok(preview) => {
                ctx.render_preview(preview)?;

                if let Some(syntax) = maybe_syntax {
                    ctx.vim.set_preview_syntax(syntax)?;
                }
            }
            Err(err) => {
                ctx.render_preview(Preview {
                    lines: vec![err.to_string()],
                    ..Default::default()
                })?;
            }
        }
        Ok(())
    }

    fn load_parent(&mut self, ctx: &Context) -> Result<()> {
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
                        .exec("clap#provider#filer#handle_error", [err.to_string()])?;
                    return Ok(());
                }
            };

            v.insert(
                entries
                    .into_iter()
                    .map(|line| {
                        let item: Arc<dyn ClapItem> = Arc::new(FilerItem(line));
                        item
                    })
                    .collect(),
            );
        }

        Ok(())
    }
}

#[async_trait::async_trait]
impl ClapProvider for FilerProvider {
    async fn on_create(&mut self, ctx: &mut Context) -> Result<()> {
        let cwd = &ctx.cwd;

        let entries = match read_dir_entries(cwd, ctx.env.icon.enabled(), None) {
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
            .exec("clap#provider#filer#handle_on_create", response)?;

        self.dir_entries.insert(
            cwd.to_path_buf(),
            entries
                .clone()
                .into_iter()
                .map(|line| {
                    let item: Arc<dyn ClapItem> = Arc::new(FilerItem(line));
                    item
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
        let curline = self.current_line(ctx).await?;
        let path = build_abs_path(&self.current_dir, curline);
        let preview_target = if path.is_dir() {
            PreviewTarget::Directory(path)
        } else {
            PreviewTarget::File(path)
        };
        let preview_height = ctx.preview_height().await?;
        self.update_preview(preview_target, preview_height, ctx)
            .await
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
        match key_event {
            KeyEvent::Tab => self.on_tab(ctx).await,
            KeyEvent::Backspace => self.on_backspace(ctx).await,
            KeyEvent::CarriageReturn => self.on_carriage_return(ctx).await,
            KeyEvent::ShiftUp => {
                tracing::debug!("TODO: ShiftUp, Preview scroll up");
                Ok(())
            }
            KeyEvent::ShiftDown => {
                tracing::debug!("TODO: ShiftDown, Preview scroll down");
                Ok(())
            }
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
            &std::env::current_dir()
                .unwrap()
                .into_os_string()
                .into_string()
                .unwrap(),
            false,
            None,
        )
        .unwrap();

        assert_eq!(entries, vec!["Cargo.toml", "benches/", "src/"]);
    }
}
