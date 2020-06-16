use super::SessionContext;
use crate::types::{Message, ProviderId};
use anyhow::{anyhow, Result};
use serde_json::json;
use std::path::PathBuf;

pub enum OnInit {
    Filer(PathBuf),
}

pub struct OnInitHandler {
    pub msg_id: u64,
    pub provider_id: ProviderId,
    pub size: usize,
    pub inner: OnInit,
}

impl OnInitHandler {
    pub fn try_new(msg: Message, context: &SessionContext) -> Result<Self> {
        let msg_id = msg.id;
        let provider_id = context.provider_id.clone();
        if provider_id.as_str() == "filer" {
            let path = &msg
                .get_cwd()
                .ok_or(anyhow!("Missing cwd in message.params"))?;
            return Ok(Self {
                msg_id,
                size: provider_id.get_preview_size(),
                provider_id,
                inner: OnInit::Filer(path.into()),
            });
        }
        // TODO: filer does not have curline
        let curline = msg.get_curline(&provider_id)?;
        Err(anyhow!("Currently only filer"))
    }

    pub fn handle(&self) {
        match &self.inner {
            OnInit::Filer(cwd) => {
                let result = match crate::filer::read_dir_entries(
                    cwd,
                    crate::env::global().enable_icon,
                    None,
                ) {
                    Ok(entries) => {
                        let result = json!({
                        "event:": "on_init",
                        "entries": entries,
                        "dir": cwd,
                        "total": entries.len(),
                        });
                        json!({ "id": self.msg_id, "provider_id": "filer", "result": result })
                    }
                    Err(err) => {
                        let error = json!({"message": format!("{}", err), "dir": cwd});
                        json!({ "id": self.msg_id, "provider_id": "filer", "error": error })
                    }
                };

                crate::write_response(result);
            }
        }
    }
}
