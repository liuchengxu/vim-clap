use std::path::PathBuf;
use std::sync::{atomic::AtomicBool, Arc};

use anyhow::Result;
use parking_lot::Mutex;
use serde::Deserialize;

use crate::stdio_server::{types::ProviderId, Message};

const DEFAULT_DISPLAY_WINWIDTH: u64 = 100;

const DEFAULT_PREVIEW_WINHEIGHT: u64 = 30;

/// This type represents the scale of filtering source.
#[derive(Debug, Clone)]
pub enum Scale {
    /// We do not know the exact total number of source items.
    Indefinite,
    /// Large scale.
    Large(usize),
    /// Small scale.
    Small(usize),
}

impl Default for Scale {
    fn default() -> Self {
        Self::Indefinite
    }
}

impl Scale {
    pub fn total(&self) -> Option<usize> {
        match self {
            Self::Large(total) | Self::Small(total) => Some(*total),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SessionContext {
    pub provider_id: ProviderId,
    pub cwd: PathBuf,
    pub start_buffer_path: PathBuf,
    pub display_winwidth: u64,
    pub preview_winheight: u64,
    pub source_cmd: Option<String>,
    pub scale: Arc<Mutex<Scale>>,
    pub runtimepath: Option<String>,
    pub is_running: Arc<Mutex<AtomicBool>>,
    pub source_list: Arc<Mutex<Option<Vec<String>>>>,
}

impl SessionContext {
    /// Executes the command `cmd` and returns the raw bytes of stdout.
    pub fn execute(&self, cmd: &str) -> Result<Vec<u8>> {
        let out = utility::execute_at(cmd, Some(&self.cwd))?;
        Ok(out.stdout)
    }

    /// Size for fulfilling the preview window.
    pub fn sensible_preview_size(&self) -> usize {
        std::cmp::max(
            self.provider_id.get_preview_size(),
            (self.preview_winheight / 2) as usize,
        )
    }
}

impl From<Message> for SessionContext {
    fn from(msg: Message) -> Self {
        log::debug!("Creating a new SessionContext from: {:?}", msg);

        #[derive(Deserialize)]
        struct Params {
            provider_id: ProviderId,
            cwd: PathBuf,
            source_fpath: PathBuf,
            display_winwidth: Option<u64>,
            preview_winheight: Option<u64>,
            source_cmd: Option<String>,
            runtimepath: Option<String>,
        }

        let Params {
            provider_id,
            cwd,
            source_fpath,
            display_winwidth,
            preview_winheight,
            source_cmd,
            runtimepath,
        } = msg.deserialize_params_unsafe();

        Self {
            provider_id,
            cwd,
            start_buffer_path: source_fpath,
            display_winwidth: display_winwidth.unwrap_or(DEFAULT_DISPLAY_WINWIDTH),
            preview_winheight: preview_winheight.unwrap_or(DEFAULT_PREVIEW_WINHEIGHT),
            source_cmd,
            runtimepath,
            scale: Arc::new(Mutex::new(Scale::Indefinite)),
            is_running: Arc::new(Mutex::new(true.into())),
            source_list: Arc::new(Mutex::new(None)),
        }
    }
}
