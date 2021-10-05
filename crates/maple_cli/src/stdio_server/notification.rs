use anyhow::{anyhow, Result};
use jsonrpc_core::Params;
use serde::{Deserialize, Serialize};

use crate::datastore::RECENT_FILES_IN_MEMORY;

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Notification {
    pub method: String,
    pub params: Params,
    pub session_id: u64,
}

impl Notification {
    pub async fn handle(&self) -> Result<()> {
        match self.method.as_str() {
            "note_recent_files" => self.handle_note_recent_file().await,
            _ => {
                Err(anyhow!("Unknown notification: {:?}", self))
            }
        }
    }

    async fn handle_note_recent_file(&self) -> Result<()> {
        #[derive(serde::Deserialize)]
        struct InnerParams {
            file: String,
        }

        let InnerParams { file } = self.params.clone().parse()?;

        if file.is_empty() || !std::path::Path::new(&file).exists() {
            return Ok(());
        }

        let mut recent_files = RECENT_FILES_IN_MEMORY.lock();
        recent_files.upsert(file);

        Ok(())
    }
}
