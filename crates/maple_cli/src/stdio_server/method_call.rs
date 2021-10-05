use jsonrpc_core::Params;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::{json, Value};

use crate::stdio_server::types::ProviderId;

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MethodCall {
    pub id: u64,
    pub method: String,
    pub params: Params,
    pub session_id: u64,
}

impl MethodCall {
    pub fn parse<T: DeserializeOwned>(self) -> anyhow::Result<T> {
        self.params.parse().map_err(Into::into)
    }

    pub fn parse_unsafe<T: DeserializeOwned>(self) -> T {
        self.params
            .parse()
            .unwrap_or_else(|e| panic!("Couldn't deserialize params: {:?}", e))
    }

    pub fn get_query(&self) -> String {
        self.get_string_unsafe("query")
    }

    pub fn get_cwd(&self) -> String {
        self.get_string_unsafe("cwd")
    }

    /// Get the current line of display window without the leading icon.
    pub fn get_curline(&self, provider_id: &ProviderId) -> anyhow::Result<String> {
        let display_curline = self.get_string("curline")?;

        let curline = if let Ok(enable_icon) = self.get_bool("enable_icon") {
            if enable_icon {
                display_curline.chars().skip(2).collect()
            } else {
                display_curline
            }
        } else if provider_id.should_skip_leading_icon() {
            display_curline.chars().skip(2).collect()
        } else {
            display_curline
        };

        Ok(curline)
    }

    fn map_params(&self) -> anyhow::Result<&serde_json::Map<String, Value>> {
        match &self.params {
            Params::None => Err(anyhow::anyhow!("None params unsupported")),
            Params::Array(_) => Err(anyhow::anyhow!("Array params unsupported")),
            Params::Map(map) => Ok(map),
        }
    }

    #[allow(unused)]
    pub fn get_u64(&self, key: &str) -> anyhow::Result<u64> {
        self.map_params()?
            .get(key)
            .and_then(|x| x.as_u64())
            .ok_or_else(|| anyhow::anyhow!("Missing {} in msg.params", key))
    }

    pub fn get_str(&self, key: &str) -> anyhow::Result<&str> {
        self.map_params()?
            .get(key)
            .and_then(|x| x.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing {} in msg.params", key))
    }

    pub fn get_string(&self, key: &str) -> anyhow::Result<String> {
        self.get_str(key).map(Into::into)
    }

    pub fn get_string_unsafe(&self, key: &str) -> String {
        self.get_string(key)
            .unwrap_or_else(|e| panic!("Get String error: {:?}", e))
    }

    pub fn get_bool(&self, key: &str) -> anyhow::Result<bool> {
        self.map_params()?
            .get(key)
            .and_then(|x| x.as_bool())
            .ok_or_else(|| anyhow::anyhow!("Missing {} in msg.params", key))
    }
}

impl MethodCall {
    pub fn handle(self) -> anyhow::Result<Value> {
        use super::dumb_jump::DumbJumpSession;
        use super::recent_files::RecentFilesSession;
        use super::SessionEvent::*;

        let msg = self;

        if msg.method != "init_ext_map" {
            log::debug!("==> stdio message(in): {:?}", msg);
        }

        let value = match &msg.method[..] {
            // "init_ext_map" => super::message_handlers::parse_filetypedetect(msg),
            // "preview/file" => super::message_handlers::preview_file(msg),
            // "quickfix" => super::quickfix::preview_quickfix_entry(msg),

            /*
            "dumb_jump/on_init" => manager.new_session::<DumbJumpSession>(msg),
            "dumb_jump/on_typed" => manager.send(msg.session_id, OnTyped(msg)),
            "dumb_jump/on_move" => manager.send(msg.session_id, OnMove(msg)),

            "recent_files/on_init" => manager.new_session::<RecentFilesSession>(msg),
            "recent_files/on_typed" => manager.send(msg.session_id, OnTyped(msg)),
            "recent_files/on_move" => manager.send(msg.session_id, OnMove(msg)),

            "filer" => filer::handle_filer_message(msg),
            "filer/on_init" => manager.new_session::<FilerSession>(msg),
            "filer/on_move" => manager.send(msg.session_id, OnMove(msg)),

            "on_init" => manager.new_session::<BuiltinSession>(msg),
            "on_typed" => manager.send(msg.session_id, OnTyped(msg)),
            "on_move" => manager.send(msg.session_id, OnMove(msg)),
            "exit" => manager.terminate(msg.session_id),
            */
            _ => json!({ "error": format!("unknown method: {}", &msg.method[..]), "id": msg.id }),
        };

        Ok(value)
    }
}
