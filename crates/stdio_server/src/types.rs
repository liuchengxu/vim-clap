use anyhow::Context;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone)]
pub struct GlobalEnv {
    pub is_nvim: bool,
    pub enable_icon: bool,
    pub preview_size: Value,
}

impl GlobalEnv {
    pub fn new(is_nvim: bool, enable_icon: bool, preview_size: Value) -> Self {
        Self {
            is_nvim,
            enable_icon,
            preview_size,
        }
    }

    pub fn preview_size_of(&self, provider_id: &str) -> usize {
        match self.preview_size {
            Value::Number(ref number) => number.as_u64().unwrap() as usize,
            Value::Object(ref obj) => {
                let get_size = |key: &str| {
                    obj.get(key)
                        .and_then(|x| x.as_u64().map(|i| i as usize))
                        .unwrap()
                };
                if obj.contains_key(provider_id) {
                    get_size(provider_id)
                } else if obj.contains_key("*") {
                    get_size("*")
                } else {
                    5usize
                }
            }
            _ => unreachable!("clap_preview_size has to be either Number or Object"),
        }
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
    pub fn get_message_id(&self) -> u64 {
        self.id
    }

    pub fn get_provider_id(&self) -> String {
        self.params
            .get("provider_id")
            .and_then(|x| x.as_str())
            .unwrap_or("Unknown provider id")
            .into()
    }

    #[allow(dead_code)]
    pub fn get_query(&self) -> String {
        self.params
            .get("query")
            .and_then(|x| x.as_str())
            .expect("Unknown provider id")
            .into()
    }

    /// Get the current line of display window without the leading icon.
    pub fn get_curline(&self, provider_id: &str) -> anyhow::Result<String> {
        let display_curline = String::from(
            self.params
                .get("curline")
                .and_then(|x| x.as_str())
                .context("Missing fname when deserializing into FilerParams")?,
        );

        let curline = if super::env::should_skip_leading_icon(provider_id) {
            display_curline.chars().skip(2).collect()
        } else {
            display_curline
        };

        Ok(curline)
    }
}
