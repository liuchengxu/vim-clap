use std::collections::HashMap;

use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone)]
pub struct GlobalEnv {
    pub is_nvim: bool,
    pub enable_icon: bool,
    pub preview_config: PreviewConfig,
}

#[derive(Debug, Clone)]
pub enum PreviewConfig {
    Number(u64),
    Map(HashMap<String, u64>),
}

const DEFAULT_PREVIEW_SIZE: u64 = 5;

impl From<Value> for PreviewConfig {
    fn from(v: Value) -> Self {
        if v.is_object() {
            let m: HashMap<String, u64> = serde_json::from_value(v)
                .unwrap_or_else(|e| panic!("Failed to deserialize preview_size map: {:?}", e));
            return Self::Map(m);
        }
        match v {
            Value::Number(number) => Self::Number(number.as_u64().unwrap_or(DEFAULT_PREVIEW_SIZE)),
            _ => unreachable!("clap_preview_size has to be either Number or Object"),
        }
    }
}

impl PreviewConfig {
    pub fn preview_size(&self, provider_id: &str) -> usize {
        match self {
            Self::Number(n) => *n as usize,
            Self::Map(map) => map
                .get(provider_id)
                .map(|x| *x)
                .unwrap_or_else(|| map.get("*").copied().unwrap_or(DEFAULT_PREVIEW_SIZE))
                as usize,
        }
    }
}

impl GlobalEnv {
    pub fn new(is_nvim: bool, enable_icon: bool, preview_config: PreviewConfig) -> Self {
        Self {
            is_nvim,
            enable_icon,
            preview_config,
        }
    }

    /// Each provider can have its preferred preview size.
    pub fn preview_size_of(&self, provider_id: &str) -> usize {
        self.preview_config.preview_size(provider_id)
    }
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Message {
    pub method: String,
    pub params: serde_json::Map<String, Value>,
    pub id: u64,
    pub session_id: u64,
}

impl Message {
    pub fn deserialize_params<T: DeserializeOwned>(self) -> anyhow::Result<T> {
        let json_value = serde_json::Value::Object(self.params);
        serde_json::from_value(json_value).map_err(Into::into)
    }

    pub fn get_provider_id(&self) -> ProviderId {
        self.get_string_unsafe("provider_id").into()
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

    pub fn get_u64(&self, key: &str) -> anyhow::Result<u64> {
        self.params
            .get(key)
            .and_then(|x| x.as_u64())
            .ok_or_else(|| anyhow::anyhow!("Missing {} in msg.params", key))
    }

    pub fn get_str(&self, key: &str) -> anyhow::Result<&str> {
        self.params
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
        self.params
            .get(key)
            .and_then(|x| x.as_bool())
            .ok_or_else(|| anyhow::anyhow!("Missing {} in msg.params", key))
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ProviderId(String);

const NO_ICON_PROVIDERS: [&str; 5] = ["blines", "commits", "bcommits", "help_tags", "dumb_jump"];

impl ProviderId {
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Returns true if the raw line has been decorated with an icon.
    ///
    /// We should skip that icon when hoping to get the origin cursorline content.
    #[inline]
    pub fn should_skip_leading_icon(&self) -> bool {
        super::global().enable_icon && self.has_icon_support()
    }

    /// Returns the preview size of current provider.
    #[inline]
    pub fn get_preview_size(&self) -> usize {
        super::global().preview_size_of(&self.0)
    }

    /// Returns true if the provider can have icon.
    #[inline]
    pub fn has_icon_support(&self) -> bool {
        !NO_ICON_PROVIDERS.contains(&self.as_str())
    }
}

impl From<String> for ProviderId {
    fn from(p: String) -> Self {
        Self(p)
    }
}

impl From<&str> for ProviderId {
    fn from(p: &str) -> Self {
        Self(p.into())
    }
}

impl std::fmt::Display for ProviderId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_preview_config_deserialize() {
        let v: Value = serde_json::json!({"filer": 10, "files": 5});
        let _config: PreviewConfig = v.into();
    }
}
