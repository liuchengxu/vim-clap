use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf, MAIN_SEPARATOR};
use std::sync::Arc;

use anyhow::Result;
use serde_json::json;

use icon::prepend_filer_icon;
use types::{ClapItem, MatchResult};

use crate::stdio_server::handler::{OnMoveHandler, PreviewKind};
use crate::stdio_server::provider::{ClapProvider, ProviderContext};
use crate::stdio_server::vim::Vim;
use crate::utils::build_abs_path;

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
                write!(f, "{}", path)
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
        format!("{}{}", parent_dir.display(), std::path::MAIN_SEPARATOR)
    };

    if let Some(last_char) = cur_dir.chars().last() {
        if last_char == std::path::MAIN_SEPARATOR {}
    }
}

pub fn read_dir_entries<P: AsRef<Path>>(
    dir: P,
    enable_icon: bool,
    max: Option<usize>,
) -> std::io::Result<Vec<String>> {
    let entries_iter = fs::read_dir(dir)?
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
    context: ProviderContext,
    current_dir: PathBuf,
    dir_entries: HashMap<PathBuf, Vec<Arc<dyn ClapItem>>>,
    current_lines: Vec<String>,
}

impl FilerProvider {
    pub fn new(context: ProviderContext) -> Self {
        Self {
            current_dir: context.cwd.clone(),
            context,
            dir_entries: HashMap::new(),
            current_lines: Vec::new(),
        }
    }

    #[inline]
    fn vim(&self) -> &Vim {
        &self.context.vim
    }

    // Without the icon.
    async fn current_line(&self) -> Result<String> {
        let curline = self.vim().display_getcurline().await?;
        let curline = if self.vim().get_var_bool("clap_enable_icon").await? {
            curline.chars().skip(2).collect()
        } else {
            curline
        };
        Ok(curline)
    }

    fn on_query_change(&self, query: &str) -> Result<Vec<String>> {
        let current_items = self
            .dir_entries
            .get(&self.current_dir)
            .ok_or_else(|| anyhow::anyhow!("Directory entries not found"))?;

        let matched_items = filter::par_filter_items(query, current_items, &self.context.matcher);
        let total = matched_items.len();

        let printer::DisplayLines {
            lines,
            indices,
            truncated_map,
            icon_added,
        } = printer::decorate_lines(
            matched_items.iter().take(200).cloned().collect(),
            self.context.display_winwidth,
            icon::Icon::Null, // icon is handled inside the provider impl.
        );

        let result = if truncated_map.is_empty() {
            json!({ "lines": &lines, "indices": indices, "total": total, "icon_added": icon_added })
        } else {
            json!({ "lines": &lines, "indices": indices, "total": total, "icon_added": icon_added, "truncated_map": truncated_map })
        };

        self.vim()
            .exec("clap#state#process_filter_message", json!([result, true]))?;

        Ok(lines)
    }

    fn reset_to(&mut self, dir: PathBuf) -> Result<()> {
        self.current_dir = dir.clone();
        self.load_dir(dir)?;
        self.vim().exec("input_set", json!([""]))?;
        self.vim()
            .exec("clap#provider#filer#set_prompt", json!([&self.current_dir]))?;
        let lines = self.on_query_change("")?;
        self.current_lines = lines;
        Ok(())
    }

    async fn do_preview(&self, preview_kind: PreviewKind) -> Result<()> {
        let on_move_handler = OnMoveHandler {
            size: self
                .vim()
                .preview_size(&self.context.provider_id, self.context.preview.winid)
                .await?,
            context: &self.context,
            preview_kind,
            cache_line: None,
        };
        let preview = on_move_handler.get_preview().await?;
        self.vim()
            .exec("clap#state#process_preview_result", preview)?;
        Ok(())
    }

    fn load_parent(&mut self) -> Result<()> {
        let parent_dir = match self.current_dir.parent() {
            Some(parent) => parent,
            None => return Ok(()),
        };
        self.current_dir = parent_dir.to_path_buf();
        self.load_dir(self.current_dir.clone())
    }

    fn load_dir(&mut self, target_dir: PathBuf) -> Result<()> {
        if let Entry::Vacant(v) = self.dir_entries.entry(target_dir) {
            let entries =
                match read_dir_entries(&self.current_dir, self.context.icon.enabled(), None) {
                    Ok(entries) => entries,
                    Err(err) => {
                        self.vim()
                            .exec("clap#provider#filer#handle_error", json!([err.to_string()]))?;
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
    fn context(&self) -> &ProviderContext {
        &self.context
    }

    async fn on_create(&mut self) -> Result<()> {
        let cwd = &self.context.cwd;

        let entries = match read_dir_entries(cwd, self.context.icon.enabled(), None) {
            Ok(entries) => entries,
            Err(err) => {
                tracing::error!(?cwd, "Failed to read directory entries");
                self.vim()
                    .exec("clap#provider#filer#handle_error", json!([err.to_string()]))?;
                return Ok(());
            }
        };

        let response = json!({ "entries": &entries, "dir": cwd, "total": entries.len() });
        self.vim()
            .exec("clap#provider#filer#handle_on_create", response)?;

        self.dir_entries.insert(
            cwd.clone(),
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

    async fn on_move(&mut self) -> Result<()> {
        let curline = self.current_line().await?;
        let path = build_abs_path(&self.current_dir, curline);
        self.do_preview(PreviewKind::FileOrDirectory(path.clone()))
            .await?;
        Ok(())
    }

    async fn on_typed(&mut self) -> Result<()> {
        if self.current_lines.is_empty() {
            self.vim()
                .exec("clap#provider#filer#set_create_file_entry", json!([]))?;
            return Ok(());
        }

        let query: String = self.vim().input_get().await?;
        let lines = self.on_query_change(&query)?;
        self.current_lines = lines;

        if self.current_lines.is_empty() {
            self.vim()
                .exec("clap#provider#filer#set_create_file_entry", json!([]))?;
        }

        Ok(())
    }

    async fn tab(&mut self) -> Result<()> {
        // Most providers don't need this, hence a default impl is provided.
        let mut target_dir = self.current_dir.clone();
        let curline = self.current_line().await?;

        target_dir.push(curline);

        if target_dir.is_dir() {
            self.reset_to(target_dir)?;

            let curline = self.current_line().await?;
            self.do_preview(PreviewKind::FileOrDirectory(curline.into()))
                .await?;
        } else if target_dir.is_file() {
            self.do_preview(PreviewKind::File(target_dir.clone()))
                .await?;
        }

        Ok(())
    }

    async fn backspace(&mut self) -> Result<()> {
        let mut input = self.vim().input_get().await?;

        tracing::debug!("==================== new input: {:?}", input);
        if input.is_empty() {
            self.load_parent()?;
            self.vim()
                .exec("clap#provider#filer#set_prompt", json!([&self.current_dir]))?;
        } else {
            input.pop();
            self.vim().exec("input_set", json!([input]))?;
        }

        let lines = self.on_query_change(&input)?;
        self.current_lines = lines;

        Ok(())
    }

    async fn carriage_return(&mut self) -> Result<()> {
        let mut target_dir = self.current_dir.clone();
        let curline = self.current_line().await?;

        target_dir.push(curline);

        if target_dir.is_dir() {
            self.reset_to(target_dir)?;
            return Ok(());
        } else if target_dir.is_file() {
            self.vim().exec("execute", json!(["stopinsert"]))?;
            self.vim()
                .exec("clap#provider#filer#sink", json!([target_dir]))?;
            return Ok(());
        }

        let mut target_file = self.current_dir.clone();
        let input = self.vim().input_get().await?;
        target_file.push(input);

        let handle_special_entries_is_ok: bool = self
            .vim()
            .call(
                "clap#provider#filer#handle_special_entries",
                json!([target_file]),
            )
            .await?;

        if handle_special_entries_is_ok {
            return Ok(());
        }

        Ok(())
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
