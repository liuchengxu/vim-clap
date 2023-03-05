use crate::datastore::RECENT_FILES_IN_MEMORY;
use crate::stdio_server::rpc::Params;
use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Notification {
    pub method: String,
    pub params: Params,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<u64>,
}

impl Notification {
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
