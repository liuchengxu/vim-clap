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

    /// Each provider can have its preferred preview size.
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
    pub fn get_provider_id(&self) -> ProviderId {
        self._get_string_unsafe("provider_id").into()
    }

    #[allow(dead_code)]
    pub fn get_query(&self) -> String {
        self._get_string_unsafe("query")
    }

    pub fn get_cwd(&self) -> String {
        self._get_string_unsafe("cwd")
    }

    /// Get the current line of display window without the leading icon.
    pub fn get_curline(&self, provider_id: &ProviderId) -> anyhow::Result<String> {
        let display_curline = self._get_string("curline")?;

        let curline = if provider_id.should_skip_leading_icon() {
            display_curline.chars().skip(2).collect()
        } else {
            display_curline
        };

        Ok(curline)
    }

    fn _get_string_unsafe(&self, key: &str) -> String {
        self.params
            .get(key)
            .and_then(|x| x.as_str())
            .map(Into::into)
            .expect(&format!("Missing {} in msg.params", key))
    }

    fn _get_string(&self, key: &str) -> anyhow::Result<String> {
        self.params
            .get(key)
            .and_then(|x| x.as_str())
            .map(Into::into)
            .ok_or(anyhow::anyhow!("Missing {} in msg.params", key))
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ProviderId(String);

const NO_ICON_PROVIDERS: [&str; 3] = ["blines", "commits", "bcommits"];

impl ProviderId {
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Returns true if the raw line has been decorated with an icon.
    ///
    /// We should skip that icon when hoping to get the origin cursorline content.
    #[inline]
    pub fn should_skip_leading_icon(&self) -> bool {
        super::env::global().enable_icon && self.has_icon_support()
    }

    /// Returns the preview size of current provider.
    #[inline]
    pub fn get_preview_size(&self) -> usize {
        super::env::global().preview_size_of(&self.0)
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
