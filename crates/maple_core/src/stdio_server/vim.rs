use crate::paths::AbsPathBuf;
use crate::stdio_server::provider::ProviderId;
use anyhow::{anyhow, Result};
use once_cell::sync::{Lazy, OnceCell};
use printer::DisplayLines;
use rayon::prelude::*;
use rpc::RpcClient;
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::ops::Deref;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use types::ProgressUpdate;

static FILENAME_SYNTAX_MAP: Lazy<HashMap<&str, &str>> = Lazy::new(|| {
    vec![
        ("bashrc", "bash"),
        (".bashrc", "bash"),
        ("BUCK", "bzl"),
        ("BUILD", "bzl"),
        ("BUILD.bazel", "bzl"),
        ("Tiltfile", "bzl"),
        ("WORKSPACE", "bz"),
        ("configure.ac", "config"),
        ("configure.in", "config"),
        ("Containerfile", "dockerfile"),
        ("Dockerfile", "dockerfile"),
        ("dockerfile", "dockerfile"),
        ("jsconfig.json", "jsonc"),
        ("tsconfig.json", "jsonc"),
        ("mplayer.conf", "mplayerconf"),
        ("inputrc", "readline"),
        ("robots.txt", "robots"),
        ("ssh_config", "sshdconfig"),
        ("sshd_config", "sshdconfig"),
        ("tidy.conf", "tidy"),
        ("tidyrc", "tidy"),
        ("Pipfile", "toml"),
        ("vimrc", "vim"),
        ("_vimrc", "vim"),
        ("_viminfo", "viminfo"),
    ]
    .into_iter()
    .collect()
});

/// Map of file extension to vim syntax mapping.
static SYNTAX_MAP: OnceCell<HashMap<String, String>> = OnceCell::new();

pub fn initialize_syntax_map(output: &str) -> HashMap<&str, &str> {
    let ext_map: HashMap<&str, &str> = output
        .par_split(|x| x == '\n')
        .filter(|s| s.contains("setf"))
        .filter_map(|s| {
            // *.mkiv    setf context
            let items = s.split_whitespace().collect::<Vec<_>>();
            if items.len() != 3 {
                None
            } else {
                // (mkiv, context)
                items[0].split('.').last().map(|ext| (ext, items[2]))
            }
        })
        .chain(
            // Lines as followed can not be parsed correctly, thus the preview highlight of
            // related file will be broken. Ref #800
            // *.c       call dist#ft#FTlpc()
            vec![
                ("hpp", "cpp"),
                ("vimrc", "vim"),
                ("cc", "cpp"),
                ("cpp", "cpp"),
                ("c", "c"),
                ("h", "c"),
                ("cmd", "dosbatch"),
                ("CMakeLists.txt", "cmake"),
                ("Dockerfile", "dockerfile"),
                ("directory", "desktop"),
                ("patch", "diff"),
                ("dircolors", "dircolors"),
                ("editorconfig", "dosini"),
                ("COMMIT_EDITMSG", "gitcommit"),
                ("MERGE_MSG", "gitcommit"),
                ("TAG_EDITMSG", "gitcommit"),
                ("NOTES_EDITMSG", "gitcommit"),
                ("EDIT_DESCRIPTION", "gitcommit"),
                ("gitconfig", "gitconfig"),
                ("worktree", "gitconfig"),
                ("gitmodules", "gitconfig"),
                ("htm", "html"),
                ("html", "html"),
                ("shtml", "html"),
                ("stm", "html"),
                ("toml", "toml"),
            ]
            .into_par_iter(),
        )
        .map(|(ext, ft)| (ext, ft))
        .collect();

    if let Err(e) = SYNTAX_MAP.set(
        ext_map
            .par_iter()
            .map(|(k, v)| (String::from(*k), String::from(*v)))
            .collect(),
    ) {
        tracing::debug!(error = ?e, "Failed to initialized SYNTAX_MAP");
    } else {
        tracing::debug!("SYNTAX_MAP initialized successfully");
    }

    ext_map
}

/// Returns the value of `&syntax` for given path for the preview buffer highlight.
///
/// Try the file name first and then the file extension.
pub fn preview_syntax(path: &Path) -> Option<&str> {
    match path
        .file_name()
        .and_then(|x| x.to_str())
        .and_then(|filename| FILENAME_SYNTAX_MAP.deref().get(filename))
    {
        None => path
            .extension()
            .and_then(|x| x.to_str())
            .and_then(|ext| SYNTAX_MAP.get().and_then(|m| m.get(ext).map(AsRef::as_ref))),
        Some(s) => Some(s),
    }
}

#[derive(Debug, Clone)]
pub enum PreviewConfig {
    Number(u64),
    Map(HashMap<String, u64>),
}

impl From<Value> for PreviewConfig {
    fn from(v: Value) -> Self {
        if v.is_object() {
            let m: HashMap<String, u64> = serde_json::from_value(v)
                .unwrap_or_else(|e| panic!("Failed to deserialize preview_size map: {e:?}"));
            return Self::Map(m);
        }
        match v {
            Value::Number(number) => {
                Self::Number(number.as_u64().unwrap_or(Self::DEFAULT_PREVIEW_SIZE))
            }
            _ => unreachable!("clap_preview_size has to be either Number or Object"),
        }
    }
}

impl PreviewConfig {
    const DEFAULT_PREVIEW_SIZE: u64 = 5;

    pub fn preview_size(&self, provider_id: &str) -> usize {
        match self {
            Self::Number(n) => *n as usize,
            Self::Map(map) => map
                .get(provider_id)
                .copied()
                .unwrap_or_else(|| map.get("*").copied().unwrap_or(Self::DEFAULT_PREVIEW_SIZE))
                as usize,
        }
    }
}

pub struct VimProgressor {
    vim: Vim,
    stopped: Arc<AtomicBool>,
}

impl VimProgressor {
    pub fn new(vim: Vim, stopped: Arc<AtomicBool>) -> Self {
        Self { vim, stopped }
    }
}

impl ProgressUpdate<DisplayLines> for VimProgressor {
    fn update_brief(&self, total_matched: usize, total_processed: usize) {
        if self.stopped.load(Ordering::Relaxed) {
            return;
        }

        let _ = self.vim.exec(
            "clap#state#process_progress",
            json!([total_matched, total_processed]),
        );
    }

    fn update_all(
        &self,
        display_lines: &DisplayLines,
        total_matched: usize,
        total_processed: usize,
    ) {
        if self.stopped.load(Ordering::Relaxed) {
            return;
        }
        let _ = self.vim.exec(
            "clap#state#process_progress_full",
            json!([display_lines, total_matched, total_processed]),
        );
    }

    fn on_finished(
        &self,
        display_lines: DisplayLines,
        total_matched: usize,
        total_processed: usize,
    ) {
        if self.stopped.load(Ordering::Relaxed) {
            return;
        }
        let _ = self.vim.exec(
            "clap#state#process_progress_full",
            json!([display_lines, total_matched, total_processed]),
        );
    }
}

// Vim may return 1/0 for true/false.
#[inline(always)]
fn from_vim_bool(value: Value) -> bool {
    match value {
        Value::Bool(b) => b,
        Value::Number(n) => n.as_u64().map(|n| n == 1).unwrap_or(false),
        _ => false,
    }
}

/// Shareable Vim instance.
#[derive(Debug, Clone)]
pub struct Vim {
    rpc_client: Arc<RpcClient>,
}

impl Vim {
    /// Constructs a [`Vim`].
    pub fn new(rpc_client: Arc<RpcClient>) -> Self {
        Self { rpc_client }
    }

    /// Calls the method with given params in Vim and return the call result.
    ///
    /// `method`: Must be a valid argument for `clap#api#call(method, args)`.
    pub async fn call<R: DeserializeOwned>(
        &self,
        method: impl AsRef<str>,
        params: impl Serialize,
    ) -> Result<R> {
        self.rpc_client
            .request(method, params)
            .await
            .map_err(|e| anyhow!("RpcError: {e:?}"))
    }

    /// Calls the method with no arguments.
    pub async fn bare_call<R: DeserializeOwned>(&self, method: impl AsRef<str>) -> Result<R> {
        self.rpc_client
            .request(method, json!([]))
            .await
            .map_err(|e| anyhow!("RpcError: {e:?}"))
    }

    /// Executes the method with given params in Vim, ignoring the call result.
    ///
    /// `method`: Same with `{func}` in `:h call()`.
    pub fn exec(&self, method: impl AsRef<str>, params: impl Serialize) -> Result<()> {
        self.rpc_client
            .notify(method, params)
            .map_err(|e| anyhow!("RpcError: {e:?}"))
    }

    /// Executes the method with no arguments.
    pub fn bare_exec(&self, method: impl AsRef<str>) -> Result<()> {
        self.rpc_client
            .notify(method, json!([]))
            .map_err(|e| anyhow!("RpcError: {e:?}"))
    }

    /// Send back the result with specified id.
    pub fn send(
        &self,
        id: u64,
        output_result: Result<impl Serialize, rpc::RpcError>,
    ) -> Result<()> {
        self.rpc_client
            .output(id, output_result)
            .map_err(|e| anyhow!("RpcError: {e:?}"))
    }

    /////////////////////////////////////////////////////////////////
    //    builtin-function-list
    /////////////////////////////////////////////////////////////////
    pub async fn bufname(&self, bufnr: usize) -> Result<String> {
        self.call("bufname", json!([bufnr])).await
    }

    pub async fn col(&self, expr: &str) -> Result<usize> {
        self.call("col", json![expr]).await
    }

    pub async fn expand(&self, string: impl AsRef<str>) -> Result<String> {
        self.call("expand", json![string.as_ref()]).await
    }

    pub async fn eval<R: DeserializeOwned>(&self, s: &str) -> Result<R> {
        self.call("eval", json!([s])).await
    }

    pub async fn line(&self, expr: &str) -> Result<usize> {
        self.call("line", json![expr]).await
    }

    pub async fn getpos(&self, expr: &str) -> Result<[usize; 4]> {
        self.call("getpos", json![expr]).await
    }

    pub async fn winwidth(&self, winid: usize) -> Result<usize> {
        let width: i32 = self.call("winwidth", json![winid]).await?;
        if width < 0 {
            Err(anyhow!("window {winid} doesn't exist"))
        } else {
            Ok(width as usize)
        }
    }

    pub async fn winheight(&self, winid: usize) -> Result<usize> {
        let height: i32 = self.call("winheight", json![winid]).await?;
        if height < 0 {
            Err(anyhow!("window {winid} doesn't exist"))
        } else {
            Ok(height as usize)
        }
    }

    pub async fn fnamemodify(&self, fname: &str, mods: &str) -> Result<String> {
        self.call("fnamemodify", json!([fname, mods])).await
    }

    pub async fn matchdelete(&self, id: i32, win: usize) -> Result<i32> {
        self.call("matchdelete", json!([id, win])).await
    }

    /////////////////////////////////////////////////////////////////
    //    Clap related APIs
    /////////////////////////////////////////////////////////////////
    /// Returns the cursor line in display window, with icon stripped.
    pub async fn display_getcurline(&self) -> Result<String> {
        let value: Value = self.bare_call("display_getcurline").await?;
        match value {
            Value::Array(arr) => {
                let icon_added_by_maple = arr[1].as_bool().unwrap_or(false);
                let curline = match arr.into_iter().next() {
                    Some(Value::String(s)) => s,
                    e => return Err(anyhow!("curline expects a String, but got {e:?}")),
                };
                if icon_added_by_maple {
                    Ok(curline.chars().skip(2).collect())
                } else {
                    Ok(curline)
                }
            }
            _ => Err(anyhow!(
                "Invalid return value of `s:api.display_getcurline()`, [String, Bool] expected"
            )),
        }
    }

    pub async fn display_getcurlnum(&self) -> Result<usize> {
        self.eval("g:clap.display.getcurlnum()").await
    }

    pub async fn input_get(&self) -> Result<String> {
        self.eval("g:clap.input.get()").await
    }

    pub async fn provider_args(&self) -> Result<Vec<String>> {
        self.bare_call("provider_args").await
    }

    pub async fn provider_id(&self) -> Result<String> {
        self.eval("g:clap.provider.id").await
    }

    pub async fn provider_raw_args(&self) -> Result<Vec<String>> {
        self.bare_call("provider_raw_args").await
    }

    pub async fn working_dir(&self) -> Result<AbsPathBuf> {
        self.bare_call("clap#rooter#working_dir").await
    }

    pub async fn context_query_or_input(&self) -> Result<String> {
        self.bare_call("context_query_or_input").await
    }

    pub async fn files_name_only(&self) -> Result<bool> {
        let context: HashMap<String, Value> = self.eval("g:clap.context").await?;
        Ok(context.contains_key("name-only"))
    }

    pub async fn current_buffer_path(&self) -> Result<String> {
        self.bare_call("current_buffer_path").await
    }

    pub async fn curbufline(&self, lnum: usize) -> Result<Option<String>> {
        self.call("curbufline", json!([lnum])).await
    }

    pub fn set_preview_syntax(&self, syntax: &str) -> Result<()> {
        self.exec("eval", [format!("g:clap.preview.set_syntax('{syntax}')")])
    }

    /////////////////////////////////////////////////////////////////
    //    General helpers
    /////////////////////////////////////////////////////////////////
    pub fn echo_info(&self, msg: &str) -> Result<()> {
        self.exec("clap#helper#echo_info", json!([msg]))
    }

    pub async fn current_winid(&self) -> Result<usize> {
        self.bare_call("win_getid").await
    }

    pub async fn current_bufnr(&self) -> Result<usize> {
        let bufnr: i32 = self.call("bufnr", json![""]).await?;
        if bufnr < 0 {
            Err(anyhow!("bufnr doesn't exist"))
        } else {
            Ok(bufnr as usize)
        }
    }

    pub async fn getcurbufline(&self, lnum: usize) -> Result<String> {
        self.call("getbufoneline", json!(["", lnum])).await
    }

    pub async fn get_var_bool(&self, var: &str) -> Result<bool> {
        let value: Value = self.call("get_var", json!([var])).await?;
        Ok(from_vim_bool(value))
    }

    pub async fn matchdelete_batch(&self, ids: Vec<i32>, win: usize) -> Result<()> {
        if self.win_is_valid(win).await? {
            self.exec("matchdelete_batch", json!([ids, win]))?;
        }
        Ok(())
    }

    /// Size for fulfilling the preview window.
    pub async fn preview_size(
        &self,
        provider_id: &ProviderId,
        preview_winid: usize,
    ) -> Result<usize> {
        let preview_winheight: usize = self.call("winheight", json![preview_winid]).await?;
        let preview_size: Value = self.call("get_var", json!(["clap_preview_size"])).await?;
        let preview_config: PreviewConfig = preview_size.into();
        Ok(preview_config
            .preview_size(provider_id.as_str())
            .max(preview_winheight / 2))
    }

    pub fn set_var(&self, var_name: &str, value: impl Serialize) -> Result<()> {
        self.exec("set_var", json!([var_name, value]))
    }

    pub async fn win_is_valid(&self, winid: usize) -> Result<bool> {
        let value: Value = self
            .call("clap#api#floating_win_is_valid", json!([winid]))
            .await?;
        Ok(from_vim_bool(value))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_preview_config_deserialize() {
        let v: Value = serde_json::json!({"filer": 10, "files": 5});
        let _config: PreviewConfig = v.into();
    }
}
