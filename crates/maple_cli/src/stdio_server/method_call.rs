use jsonrpc_core::Params;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::Value;

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
