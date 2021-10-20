use std::collections::HashMap;

use serde::{Deserialize, Serialize};
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
                .copied()
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

impl<T: AsRef<str>> From<T> for ProviderId {
    fn from(s: T) -> Self {
        Self(s.as_ref().to_owned())
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
