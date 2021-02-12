use std::path::PathBuf;
use std::sync::{atomic::AtomicBool, Arc, Mutex};

use super::*;
use crate::types::ProviderId;

const DEFAULT_DISPLAY_WINWIDTH: u64 = 100;
const DEFAULT_PREVIEW_WINHEIGHT: u64 = 30;

#[derive(Debug, Clone)]
pub struct SessionContext {
    pub provider_id: ProviderId,
    pub cwd: PathBuf,
    pub start_buffer_path: PathBuf,
    pub display_winwidth: u64,
    pub preview_winheight: u64,
    pub source_cmd: Option<String>,
    pub runtimepath: Option<String>,
    pub is_running: Arc<Mutex<AtomicBool>>,
    pub source_list: Arc<Mutex<Option<Vec<String>>>>,
}

impl SessionContext {
    // Executes the command `cmd` and returns the raw bytes of stdout.
    pub fn execute(&self, cmd: &str) -> Result<Vec<u8>> {
        let out = utility::execute_at(cmd, Some(&self.cwd))?;
        Ok(out.stdout)
    }
}

impl From<Message> for SessionContext {
    fn from(msg: Message) -> Self {
        log::debug!("recv msg for SessionContext: {:?}", msg);
        let provider_id = msg.get_provider_id();

        let cwd = msg.get_cwd().into();

        let runtimepath = msg
            .params
            .get("runtimepath")
            .and_then(|x| x.as_str().map(Into::into));

        let source_cmd = msg
            .params
            .get("source_cmd")
            .and_then(|x| x.as_str().map(Into::into));

        let display_winwidth = msg
            .params
            .get("display_winwidth")
            .and_then(|x| x.as_u64())
            .unwrap_or(DEFAULT_DISPLAY_WINWIDTH);
        let preview_winheight = msg
            .params
            .get("preview_winheight")
            .and_then(|x| x.as_u64())
            .unwrap_or(DEFAULT_PREVIEW_WINHEIGHT);

        let start_buffer_path = msg
            .params
            .get("source_fpath")
            .and_then(|x| x.as_str())
            .unwrap_or_else(|| panic!("Missing source_fpath"))
            .into();

        Self {
            provider_id,
            cwd,
            source_cmd,
            runtimepath,
            display_winwidth,
            preview_winheight,
            start_buffer_path,
            is_running: Arc::new(Mutex::new(true.into())),
            source_list: Arc::new(Mutex::new(None)),
        }
    }
}
