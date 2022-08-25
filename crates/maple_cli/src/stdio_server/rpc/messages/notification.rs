use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::datastore::RECENT_FILES_IN_MEMORY;
use crate::stdio_server::rpc::Params;
use crate::stdio_server::types::GlobalEnv;
use crate::stdio_server::vim::Vim;
use crate::stdio_server::GLOBAL_ENV;

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Notification {
    pub method: String,
    pub params: Params,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<u64>,
}

impl Notification {
    pub async fn initialize_global_env(self, vim: Vim) -> Result<()> {
        #[derive(Deserialize)]
        struct InnerParams {
            is_nvim: Option<bool>,
            enable_icon: Option<bool>,
            clap_preview_size: serde_json::Value,
        }
        let InnerParams {
            is_nvim,
            enable_icon,
            clap_preview_size,
        } = self.params.parse()?;

        let output: String = vim
            .call("execute", json!(["autocmd filetypedetect"]))
            .await?;
        let ext_map = crate::stdio_server::vim::initialize_syntax_map(&output);
        vim.exec("clap#ext#set", json![ext_map])?;

        let is_nvim = is_nvim.unwrap_or(false);
        let enable_icon = enable_icon.unwrap_or(false);

        let global_env = GlobalEnv::new(is_nvim, enable_icon, clap_preview_size.into());

        if let Err(e) = GLOBAL_ENV.set(global_env) {
            tracing::debug!(error = ?e, "Failed to initialized GLOBAL_ENV");
        } else {
            tracing::debug!("GLOBAL_ENV initialized successfully");
        }

        Ok(())
    }

    pub async fn note_recent_file(self) -> Result<()> {
        let file: Vec<String> = self.params.parse()?;
        let file = file
            .into_iter()
            .next()
            .ok_or_else(|| anyhow::anyhow!("files is empty"))?;

        tracing::debug!(?file, "Receive a recent file");
        if file.is_empty() || !std::path::Path::new(&file).exists() {
            return Ok(());
        }

        let mut recent_files = RECENT_FILES_IN_MEMORY.lock();
        recent_files.upsert(file);

        Ok(())
    }
}
