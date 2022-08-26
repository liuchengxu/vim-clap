#![allow(unused)]

use std::collections::HashMap;
use std::ops::Deref;
use std::path::Path;
use std::sync::Arc;

use anyhow::Result;
use once_cell::sync::{Lazy, OnceCell};
use rayon::prelude::*;
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::{json, Value};

use crate::stdio_server::rpc::RpcClient;

/// Map of file extension to vim syntax mapping.
static SYNTAX_MAP: OnceCell<HashMap<String, String>> = OnceCell::new();

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

/// Returns the value of `&syntax` for a specific file path.
///
/// Used to highlight the preview buffer.
pub fn syntax_for(path: &Path) -> Option<&str> {
    match path
        .file_name()
        .and_then(|x| x.to_str())
        .and_then(|filename| FILENAME_SYNTAX_MAP.deref().get(filename).copied())
    {
        None => path.extension().and_then(|x| x.to_str()).and_then(|ext| {
            SYNTAX_MAP
                .get()
                .and_then(|m| m.get(ext).map(|s| s.as_ref()))
        }),
        Some(s) => Some(s),
    }
}

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

/// Shareable Vim instance.
#[derive(Debug, Clone)]
pub struct Vim {
    rpc_client: Arc<RpcClient>,
    // Initialized only once.
    icon_enabled: Option<bool>,
}

impl Vim {
    pub fn new(rpc_client: Arc<RpcClient>) -> Self {
        Self {
            rpc_client,
            icon_enabled: None,
        }
    }

    /// Calls the method with given params in Vim and return the call result.
    ///
    /// `method`: Must be a valid argument for `clap#api#call(method, args)`.
    pub async fn call<R: DeserializeOwned>(
        &self,
        method: impl AsRef<str>,
        params: impl Serialize,
    ) -> Result<R> {
        self.rpc_client.request(method, params).await
    }

    /// Executes the method with given params in Vim, ignoring the call result.
    ///
    /// `method`: Same with `{func}` in `:h call()`.
    pub fn exec(&self, method: impl AsRef<str>, params: impl Serialize) -> Result<()> {
        self.rpc_client.notify(method, params)
    }

    /// Send back the result with specified id.
    pub fn send(&self, id: u64, output_result: Result<impl Serialize>) -> Result<()> {
        self.rpc_client.output(id, output_result)
    }

    ///////////////////////////////////////////
    //  builtin-function-list
    ///////////////////////////////////////////
    pub async fn bufname(&self, bufnr: u64) -> Result<String> {
        self.call("bufname", json!([bufnr])).await
    }

    ///////////////////////////////////////////
    //  Clap related APIs
    ///////////////////////////////////////////
    /// Returns the cursor line in display window, with icon stripped.
    pub async fn display_getcurline(&self) -> Result<String> {
        let line: String = self.call("display_getcurline", json!([])).await?;
        if self.get_var_bool("__clap_icon_added_by_maple").await? {
            Ok(line.chars().skip(2).collect())
        } else {
            Ok(line)
        }
    }

    pub async fn display_getcurlnum(&self) -> Result<u64> {
        self.call("display_getcurlnum", json!([])).await
    }

    pub async fn input_get(&self) -> Result<String> {
        self.call("input_get", json!([])).await
    }

    pub async fn provider_id(&self) -> Result<String> {
        self.call("provider_id", json!([])).await
    }

    pub async fn working_dir(&self) -> Result<String> {
        self.call("working_dir", json!([])).await
    }

    pub async fn context_query_or_input(&self) -> Result<String> {
        self.call("context_query_or_input", json!([])).await
    }

    ///////////////////////////////////////////
    //  General helpers
    ///////////////////////////////////////////
    pub async fn get_var_bool(&self, var: &str) -> Result<bool> {
        let value: Value = self.call("get_var", json!([var])).await?;
        let value = match value {
            Value::Bool(b) => b,
            Value::Number(n) => n.as_u64().map(|n| n == 1).unwrap_or(false),
            _ => false,
        };
        Ok(value)
    }
}
