use crate::stdio_server::provider::ProviderId;
use futures::Future;
use once_cell::sync::{Lazy, OnceCell};
use paths::AbsPathBuf;
use rpc::vim::RpcClient;
use rpc::Id;
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::fmt::{Debug, Display};
use std::ops::Deref;
use std::path::Path;
use std::sync::Arc;

static FILENAME_SYNTAX_MAP: Lazy<HashMap<&str, &str>> = Lazy::new(|| {
    HashMap::from([
        ("bashrc", "bash"),
        (".bashrc", "bash"),
        ("BUCK", "bzl"),
        ("BUILD", "bzl"),
        ("BUILD.bazel", "bzl"),
        ("CMakeLists.txt", "cmake"),
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
    ])
});

/// Map of file extension to vim filetype mapping.
static EXTENSION_TO_FILETYPE_MAP: OnceCell<HashMap<String, String>> = OnceCell::new();

pub fn initialize_filetype_map(output: &str) -> HashMap<&str, &str> {
    let ext_map: HashMap<&str, &str> = output
        .split('\n')
        // Only process the normal cases.
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
            [
                ("hpp", "cpp"),
                ("vimrc", "vim"),
                ("cc", "cpp"),
                ("cpp", "cpp"),
                ("c", "c"),
                ("h", "c"),
                ("cmd", "dosbatch"),
                ("Dockerfile", "dockerfile"),
                ("directory", "desktop"),
                ("patch", "diff"),
                ("dircolors", "dircolors"),
                ("editorconfig", "dosini"),
                ("worktree", "gitconfig"),
                ("gitconfig", "gitconfig"),
                ("gitmodules", "gitconfig"),
                ("MERGE_MSG", "gitcommit"),
                ("TAG_EDITMSG", "gitcommit"),
                ("NOTES_EDITMSG", "gitcommit"),
                ("COMMIT_EDITMSG", "gitcommit"),
                ("EDIT_DESCRIPTION", "gitcommit"),
                ("htm", "html"),
                ("html", "html"),
                ("shtml", "html"),
                ("stm", "html"),
                ("toml", "toml"),
            ],
        )
        .collect();

    if let Err(e) = EXTENSION_TO_FILETYPE_MAP.set(
        ext_map
            .iter()
            .map(|(k, v)| (String::from(*k), String::from(*v)))
            .collect(),
    ) {
        tracing::debug!(error = ?e, "Failed to initialized EXTENSION_TO_FILETYPE_MAP");
    } else {
        tracing::debug!("EXTENSION_TO_FILETYPE_MAP initialized successfully");
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
        None => path.extension().and_then(|x| x.to_str()).and_then(|ext| {
            EXTENSION_TO_FILETYPE_MAP
                .get()
                .and_then(|m| m.get(ext).map(AsRef::as_ref))
        }),
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

// Vim may return 1/0 for true/false.
#[inline(always)]
fn from_vim_bool(value: Value) -> bool {
    match value {
        Value::Bool(b) => b,
        Value::Number(n) => n.as_u64().map(|n| n == 1).unwrap_or(false),
        _ => false,
    }
}

#[derive(Debug, Clone)]
pub struct ScreenLinesRange {
    pub winid: usize,
    /// Absolute line number of first line visible in current window.
    pub line_start: usize,
    /// Absolute line number of last line visible in current window.
    pub line_end: usize,
}

#[derive(Debug, thiserror::Error)]
pub enum VimError {
    #[error("buffer does not exist")]
    InvalidBuffer,
    #[error("winid {0} does not exist")]
    InvalidWinid(usize),
    #[error("setvar requires an explicit scope in `var_name`")]
    InvalidVariableScope,
    #[error("`{0}` failure")]
    VimApiFailure(String),
    #[error("{0}")]
    GetDisplayCurLine(String),
    #[error(transparent)]
    IO(#[from] std::io::Error),
    #[error(transparent)]
    Rpc(#[from] rpc::RpcError),
    #[error(transparent)]
    JsonRpc(#[from] rpc::Error),
}

pub type VimResult<T> = std::result::Result<T, VimError>;

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
    ) -> VimResult<R> {
        self.rpc_client
            .request(method, params)
            .await
            .map_err(Into::into)
    }

    /// Calls the method with no arguments.
    pub async fn bare_call<R: DeserializeOwned>(&self, method: impl AsRef<str>) -> VimResult<R> {
        self.rpc_client
            .request(method, json!([]))
            .await
            .map_err(Into::into)
    }

    /// Executes the method with given params in Vim, ignoring the call result.
    ///
    /// `method`: Same with `{func}` in `:h call()`.
    pub fn exec(&self, method: impl AsRef<str>, params: impl Serialize) -> VimResult<()> {
        self.rpc_client.notify(method, params).map_err(Into::into)
    }

    /// Executes the method with no arguments.
    pub fn bare_exec(&self, method: impl AsRef<str>) -> VimResult<()> {
        self.rpc_client
            .notify(method, json!([]))
            .map_err(Into::into)
    }

    /// Send back the result with specified id.
    pub fn send_response(
        &self,
        id: Id,
        output_result: Result<impl Serialize, rpc::RpcError>,
    ) -> VimResult<()> {
        self.rpc_client
            .send_response(id, output_result)
            .map_err(Into::into)
    }

    /////////////////////////////////////////////////////////////////
    //    builtin-function-list
    /////////////////////////////////////////////////////////////////
    pub async fn bufname(&self, bufnr: usize) -> VimResult<String> {
        self.call("bufname", [bufnr]).await
    }

    pub async fn bufnr(&self, buf: impl Serialize) -> VimResult<usize> {
        let bufnr: i32 = self.call("bufnr", [buf]).await?;
        if bufnr < 0 {
            Err(VimError::InvalidBuffer)
        } else {
            Ok(bufnr as usize)
        }
    }

    pub async fn col(&self, expr: &str) -> VimResult<usize> {
        self.call("col", [expr]).await
    }

    pub async fn deletebufline(
        &self,
        buf: impl Serialize + Debug,
        first: usize,
        last: usize,
    ) -> VimResult<()> {
        let ret: u32 = self
            .call("deletebufline", json!([buf, first, last]))
            .await?;
        if ret == 1 {
            return Err(VimError::VimApiFailure(format!(
                "`deletebufline({buf:?}, {first}, {last})`"
            )));
        }
        Ok(())
    }

    pub async fn expand(&self, string: impl AsRef<str>) -> VimResult<String> {
        self.call("expand", [string.as_ref()]).await
    }

    pub async fn eval<R: DeserializeOwned>(&self, s: &str) -> VimResult<R> {
        self.call("eval", [s]).await
    }

    pub async fn fnamemodify(&self, fname: &str, mods: &str) -> VimResult<String> {
        self.call("fnamemodify", [fname, mods]).await
    }

    pub async fn has(&self, feature: impl Serialize) -> VimResult<bool> {
        self.call::<usize>("has", [feature])
            .await
            .map(|supported| supported == 1)
    }

    pub async fn getbufoneline(&self, buf: impl Serialize, lnum: usize) -> VimResult<String> {
        self.call("getbufoneline", (buf, lnum)).await
    }

    pub async fn getbufvar<R: DeserializeOwned>(
        &self,
        buf: impl Serialize,
        varname: &str,
    ) -> VimResult<R> {
        self.call("getbufvar", (buf, varname)).await
    }

    // Same semantic as `:h getbufline()`.
    pub async fn getbufline(
        &self,
        buf: impl Serialize,
        start: impl Serialize,
        end: impl Serialize,
    ) -> VimResult<Vec<String>> {
        self.call("getbufline", (buf, start, end)).await
    }

    pub async fn getpos(&self, expr: &str) -> VimResult<[usize; 4]> {
        self.call("getpos", [expr]).await
    }

    pub async fn line(&self, expr: &str) -> VimResult<usize> {
        self.call("line", [expr]).await
    }

    pub async fn matchdelete(&self, id: i32, win: usize) -> VimResult<i32> {
        self.call("matchdelete", (id, win)).await
    }

    pub fn redrawstatus(&self) -> VimResult<()> {
        self.exec("execute", ["redrawstatus"])
    }

    pub fn setbufvar(&self, bufnr: usize, varname: &str, val: impl Serialize) -> VimResult<()> {
        self.exec("setbufvar", (bufnr, varname, val))
    }

    pub async fn winwidth(&self, winid: usize) -> VimResult<usize> {
        let width: i32 = self.call("winwidth", [winid]).await?;
        if width < 0 {
            Err(VimError::InvalidWinid(winid))
        } else {
            Ok(width as usize)
        }
    }

    pub async fn winheight(&self, winid: usize) -> VimResult<usize> {
        let height: i32 = self.call("winheight", [winid]).await?;
        if height < 0 {
            Err(VimError::InvalidWinid(winid))
        } else {
            Ok(height as usize)
        }
    }

    /////////////////////////////////////////////////////////////////
    //    Clap related APIs
    /////////////////////////////////////////////////////////////////
    /// Returns the cursor line in display window, with icon stripped.
    pub async fn display_getcurline(&self) -> VimResult<String> {
        let value: Value = self.bare_call("display_getcurline").await?;
        match value {
            Value::Array(arr) => {
                let icon_added_by_maple = arr[1].as_bool().unwrap_or(false);
                let curline = match arr.into_iter().next() {
                    Some(Value::String(s)) => s,
                    e => {
                        return Err(VimError::GetDisplayCurLine(format!(
                            "curline expects a String, but got {e:?}"
                        )))
                    }
                };
                if icon_added_by_maple {
                    Ok(curline.chars().skip(2).collect())
                } else {
                    Ok(curline)
                }
            }
            _ => Err(VimError::GetDisplayCurLine(
                "Invalid return value of `s:api.display_getcurline()`, \
                [String, Bool] expected"
                    .to_string(),
            )),
        }
    }

    pub async fn display_getcurlnum(&self) -> VimResult<usize> {
        self.eval("g:clap.display.getcurlnum()").await
    }

    pub async fn input_get(&self) -> VimResult<String> {
        self.eval("g:clap.input.get()").await
    }

    pub async fn provider_args(&self) -> VimResult<Vec<String>> {
        self.bare_call("provider_args").await
    }

    pub async fn working_dir(&self) -> VimResult<AbsPathBuf> {
        self.bare_call("clap#rooter#working_dir").await
    }

    pub async fn current_buffer_path(&self) -> VimResult<String> {
        self.bare_call("current_buffer_path").await
    }

    pub async fn curbufline(&self, lnum: usize) -> VimResult<Option<String>> {
        self.call("curbufline", [lnum]).await
    }

    pub fn set_preview_syntax(&self, syntax: &str) -> VimResult<()> {
        self.exec("eval", [format!("g:clap.preview.set_syntax('{syntax}')")])
    }

    /////////////////////////////////////////////////////////////////
    //    General helpers
    /////////////////////////////////////////////////////////////////

    pub fn echo_message(&self, msg: impl AsRef<str>) -> VimResult<()> {
        self.exec("clap#helper#echo_message", [msg.as_ref()])
    }

    pub fn echo_info(&self, msg: impl AsRef<str>) -> VimResult<()> {
        self.exec("clap#helper#echo_info", [msg.as_ref()])
    }

    pub fn echo_warn(&self, msg: impl AsRef<str>) -> VimResult<()> {
        self.exec("clap#helper#echo_warn", [msg.as_ref()])
    }

    pub async fn get_screen_lines_range(&self) -> VimResult<ScreenLinesRange> {
        let (winid, line_start, line_end) = self.bare_call("get_screen_lines_range").await?;
        Ok(ScreenLinesRange {
            winid,
            line_start,
            line_end,
        })
    }

    pub async fn get_cursor_pos(&self) -> VimResult<(usize, usize, usize)> {
        self.bare_call("get_cursor_pos").await
    }

    pub async fn filetype(&self, bufnr: usize) -> VimResult<String> {
        self.getbufvar::<String>(bufnr, "&filetype").await
    }

    pub async fn bufmodified(&self, bufnr: impl Serialize) -> VimResult<bool> {
        self.getbufvar::<u32>(bufnr, "&modified")
            .await
            .map(|m| m == 1u32)
    }

    pub async fn bufabspath(&self, bufnr: impl Display) -> VimResult<String> {
        self.expand(format!("#{bufnr}:p")).await
    }

    pub async fn verbose(&self, cmd: impl AsRef<str>) -> VimResult<String> {
        self.call::<String>("verbose", [cmd.as_ref()]).await
    }

    pub async fn current_winid(&self) -> VimResult<usize> {
        self.bare_call("win_getid").await
    }

    pub async fn get_var_bool(&self, var: &str) -> VimResult<bool> {
        let value: Value = self.call("get_var", [var]).await?;
        Ok(from_vim_bool(value))
    }

    pub async fn matchdelete_batch(&self, ids: Vec<i32>, win: usize) -> VimResult<()> {
        if self.win_is_valid(win).await? {
            self.exec("matchdelete_batch", (ids, win))?;
        }
        Ok(())
    }

    /// Size for fulfilling the preview window.
    pub async fn preview_size(
        &self,
        provider_id: &ProviderId,
        preview_winid: usize,
    ) -> VimResult<usize> {
        let preview_winheight: usize = self.call("winheight", [preview_winid]).await?;
        let preview_size: Value = self.call("get_var", ["clap_preview_size"]).await?;
        let preview_config: PreviewConfig = preview_size.into();
        Ok(preview_config
            .preview_size(provider_id.as_str())
            .max(preview_winheight / 2))
    }

    pub fn set_var(&self, var_name: &str, value: impl Serialize) -> VimResult<()> {
        if ["b:", "w:", "t:", "g:", "l:", "s:"]
            .iter()
            .any(|variable_scrope| var_name.starts_with(variable_scrope))
        {
            self.exec("set_var", (var_name, value))
        } else {
            Err(VimError::InvalidVariableScope)
        }
    }

    pub fn update_lsp_status(&self, new_status: impl AsRef<str>) -> VimResult<()> {
        self.set_var("g:clap_lsp_status", new_status.as_ref())?;
        self.redrawstatus()
    }

    pub async fn win_is_valid(&self, winid: usize) -> VimResult<bool> {
        let value: Value = self.call("win_is_valid", [winid]).await?;
        Ok(from_vim_bool(value))
    }

    pub async fn buf_is_valid(&self, buf: usize) -> VimResult<bool> {
        let value: Value = self.call("buf_is_valid", [buf]).await?;
        Ok(from_vim_bool(value))
    }

    pub async fn search_with_spinner(&self, future: impl Future<Output = ()>) {
        let _ = self.bare_exec("clap#spinner#set_busy");
        future.await;
        let _ = self.bare_exec("clap#spinner#set_idle");
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
