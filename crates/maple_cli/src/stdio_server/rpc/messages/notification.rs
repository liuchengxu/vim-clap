use anyhow::{anyhow, Result};
use jsonrpc_core::Params;
use serde::{de::DeserializeOwned, Deserialize, Serialize};

use crate::datastore::RECENT_FILES_IN_MEMORY;
use crate::stdio_server::types::GlobalEnv;
use crate::stdio_server::GLOBAL_ENV;

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Notification {
    pub method: String,
    pub params: Params,
    pub session_id: u64,
}

impl Notification {
    /// Process the notification message from Vim.
    pub async fn process(self) -> Result<()> {
        match self.method.as_str() {
            "initialize_global_env" => self.initialize_global_env(), // should be called only once.
            "note_recent_files" => self.note_recent_file().await,
            _ => Err(anyhow!("Unknown notification: {:?}", self)),
        }
    }

    pub fn parse<T: DeserializeOwned>(self) -> Result<T> {
        self.params.parse().map_err(Into::into)
    }

    pub fn parse_unsafe<T: DeserializeOwned>(self) -> T {
        self.parse()
            .unwrap_or_else(|e| panic!("Couldn't deserialize params: {:?}", e))
    }

    fn initialize_global_env(self) -> Result<()> {
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

    async fn note_recent_file(self) -> Result<()> {
        #[derive(Deserialize)]
        struct InnerParams {
            file: String,
        }

        let InnerParams { file } = self.params.parse()?;

        tracing::debug!(?file, "Receive a recent file");
        if file.is_empty() || !std::path::Path::new(&file).exists() {
            return Ok(());
        }

        let mut recent_files = RECENT_FILES_IN_MEMORY.lock();
        recent_files.upsert(file);

        Ok(())
    }
}
