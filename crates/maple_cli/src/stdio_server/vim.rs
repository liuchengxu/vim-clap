use std::collections::HashMap;
use std::ops::Deref;
use std::path::Path;
use std::sync::Arc;

use anyhow::Result;
use once_cell::sync::{Lazy, OnceCell};
use rayon::prelude::*;
use serde::de::DeserializeOwned;
use serde_json::json;

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
        .map(|filename| FILENAME_SYNTAX_MAP.deref().get(filename).copied())
        .flatten()
    {
        None => path
            .extension()
            .and_then(|x| x.to_str())
            .map(|ext| {
                SYNTAX_MAP
                    .get()
                    .and_then(|m| m.get(ext).map(|s| s.as_ref()))
            })
            .flatten(),
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

#[derive(Clone)]
pub struct Vim {
    pub rpc_client: Arc<RpcClient>,
}

impl Vim {
    pub fn new(rpc_client: Arc<RpcClient>) -> Self {
        Self { rpc_client }
    }

    pub fn getbufvar<R: DeserializeOwned>(&self, bufname: &str, var: &str) -> Result<R> {
        self.rpc_client.call("getbufvar", json!([bufname, var]))
    }
}
