use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::datastore::RECENT_FILES_IN_MEMORY;
use crate::stdio_server::rpc::Params;
use crate::stdio_server::vim::Vim;

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Notification {
    pub method: String,
    pub params: Params,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<u64>,
}

impl Notification {
    pub async fn initialize(self, vim: Vim) -> Result<()> {
        let output: String = vim
            .call("execute", json!(["autocmd filetypedetect"]))
            .await?;
        let ext_map = crate::stdio_server::vim::initialize_syntax_map(&output);
        vim.exec("clap#ext#set", json![ext_map])?;

        tracing::debug!("Client initialized successfully");

        Ok(())
    }

    pub async fn note_recent_file(self) -> Result<()> {
        let file: Vec<String> = self.params.parse()?;
        let file = file
            .into_iter()
            .next()
            .ok_or_else(|| anyhow::anyhow!("file is empty"))?;

        tracing::debug!(?file, "Received a recent file notification");

        let path = std::path::Path::new(&file);
        if !path.exists() || !path.is_file() {
            return Ok(());
        }

        let mut recent_files = RECENT_FILES_IN_MEMORY.lock();
        recent_files.upsert(file);

        Ok(())
    }
}
