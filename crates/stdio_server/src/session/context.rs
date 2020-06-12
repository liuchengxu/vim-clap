use super::*;
use crate::types::ProviderId;
use std::sync::{atomic::AtomicBool, Arc, Mutex};

#[derive(Debug, Clone)]
pub struct SessionContext {
    pub cwd: String,
    pub source_cmd: Option<String>,
    pub winwidth: Option<u64>,
    pub provider_id: ProviderId,
    pub start_buffer_path: String,
    pub is_running: Arc<Mutex<AtomicBool>>,
    pub source_list: Arc<Mutex<Option<Vec<String>>>>,
}

impl From<Message> for SessionContext {
    fn from(msg: Message) -> Self {
        log::debug!("recv msg for SessionContext: {:?}", msg);
        let provider_id = msg.get_provider_id();

        let cwd = String::from(
            msg.params
                .get("cwd")
                .and_then(|x| x.as_str())
                .unwrap_or("Missing cwd when deserializing into FilerParams"),
        );

        let source_cmd = msg
            .params
            .get("source_cmd")
            .and_then(|x| x.as_str().map(Into::into));

        let winwidth = msg.params.get("winwidth").and_then(|x| x.as_u64());

        let start_buffer_path = String::from(
            msg.params
                .get("source_fpath")
                .and_then(|x| x.as_str())
                .expect("Missing source_fpath"),
        );

        Self {
            provider_id,
            cwd,
            source_cmd,
            winwidth,
            start_buffer_path,
            is_running: Arc::new(Mutex::new(true.into())),
            source_list: Arc::new(Mutex::new(None)),
        }
    }
}
