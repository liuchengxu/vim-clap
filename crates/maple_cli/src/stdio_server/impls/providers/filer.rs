use std::path::{Path, MAIN_SEPARATOR};
use std::sync::Arc;
use std::{fs, io};

use anyhow::Result;
use serde_json::{json, Value};

use icon::prepend_filer_icon;

use crate::stdio_server::impls::{OnMove, OnMoveHandler};
use crate::stdio_server::rpc::Call;
use crate::stdio_server::session::{ClapProvider, SessionContext};
use crate::stdio_server::{write_response, MethodCall};
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

/*
function! s:goto_parent() abort
  " The root directory
  if s:is_root_directory(s:current_dir)
    return
  endif

  if s:current_dir[-1:] ==# s:PATH_SEPERATOR
    let parent_dir = fnamemodify(s:current_dir, ':h:h')
  else
    let parent_dir = fnamemodify(s:current_dir, ':h')
  endif

  if s:is_root_directory(parent_dir)
    let s:current_dir = parent_dir
  else
    let s:current_dir = parent_dir.s:PATH_SEPERATOR
  endif
  call s:set_prompt()
  call s:filter_or_send_message()
endfunction
*/

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

    let new_cur_dir = if parent_dir.parent().is_none() {
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
pub struct FilerHandle {
    context: SessionContext,
}

impl FilerHandle {
    pub fn new(context: SessionContext) -> Self {
        Self { context }
    }
}

#[async_trait::async_trait]
impl ClapProvider for FilerHandle {
    fn session_context(&self) -> &SessionContext {
        &self.context
    }

    async fn on_create(&mut self, call: Call) {
        write_response(
            handle_filer_message(call.unwrap_method_call())
                .expect("Both Success and Error are returned"),
        );
    }

    async fn on_move(&mut self, msg: MethodCall) -> Result<()> {
        #[derive(serde::Deserialize)]
        struct Params {
            // curline: String,
            cwd: String,
        }
        let msg_id = msg.id;
        // Do not use curline directly.
        let curline = msg.get_curline(&self.context.provider_id)?;
        let Params { cwd } = msg.parse_unsafe();
        let path = build_abs_path(&cwd, curline);
        let on_move_handler = OnMoveHandler {
            msg_id,
            size: self.context.sensible_preview_size(),
            context: &self.context,
            inner: OnMove::Filer(path.clone()),
            cache_line: None,
        };
        if let Err(err) = on_move_handler.handle().await {
            tracing::error!(?err, ?path, "Failed to handle filer OnMove");
            let res = json!({
              "id": msg_id,
              "provider_id": "filer",
              "error": { "message": err.to_string(), "dir": path }
            });
            write_response(res);
        }
        Ok(())
    }

    async fn on_typed(&mut self, msg: MethodCall) -> Result<()> {
        write_response(handle_filer_message(msg).expect("Both Success and Error are returned"));
        Ok(())
    }
}

fn handle_filer_message(msg: MethodCall) -> std::result::Result<Value, Value> {
    let cwd = msg.get_cwd();

    read_dir_entries(&cwd, crate::stdio_server::global().enable_icon, None)
        .map(|entries| {
            let result = json!({ "entries": entries, "dir": cwd, "total": entries.len() });
            json!({ "id": msg.id, "provider_id": "filer", "result": result })
        })
        .map_err(|err| {
            tracing::error!(?cwd, "Failed to read directory entries");
            let error = json!({"message": err.to_string(), "dir": cwd});
            json!({ "id": msg.id, "provider_id": "filer", "message": error })
        })
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
