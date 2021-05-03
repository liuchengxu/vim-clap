use std::borrow::Cow;

use serde::Deserialize;

use super::stats::Stats;

#[derive(Deserialize, Debug, Clone)]
#[serde(tag = "type", content = "data")]
#[serde(rename_all = "snake_case")]
pub enum Message {
    Begin(Begin),
    End(End),
    Match(Match),
    Context(Context),
}

#[derive(Deserialize, Debug, Clone)]
pub struct Begin {
    pub path: Data,
}

#[derive(Deserialize, Debug, Clone)]
pub struct End {
    pub path: Data,
    pub binary_offset: Option<u64>,
    pub stats: Stats,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Match {
    pub path: Data,
    pub lines: Data,
    pub line_number: Option<u64>,
    pub absolute_offset: u64,
    pub submatches: Vec<SubMatch>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Context {
    pub path: Data,
    pub lines: Data,
    pub line_number: Option<u64>,
    pub absolute_offset: u64,
    pub submatches: Vec<SubMatch>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct SubMatch {
    #[serde(rename = "match")]
    pub m: Data,
    pub start: usize,
    pub end: usize,
}

/// Copied from https://github.com/BurntSushi/ripgrep/blob/ce4b587055/crates/printer/src/jsont.rs#L82
///
/// Data represents things that look like strings, but may actually not be
/// valid UTF-8. To handle this, `Data` is serialized as an object with one
/// of two keys: `text` (for valid UTF-8) or `bytes` (for invalid UTF-8).
///
/// The happy path is valid UTF-8, which streams right through as-is, since
/// it is natively supported by JSON. When invalid UTF-8 is found, then it is
/// represented as arbitrary bytes and base64 encoded.
#[derive(Debug, Clone, Hash, PartialEq, Eq, Deserialize)]
#[serde(untagged)]
pub enum Data {
    Text {
        text: String,
    },
    Bytes {
        #[serde(deserialize_with = "from_base64")]
        bytes: Vec<u8>,
    },
}

impl Data {
    pub fn text(&self) -> Cow<str> {
        match self {
            Self::Text { text } => text.as_str().into(),
            Self::Bytes { bytes } => String::from_utf8_lossy(bytes),
        }
    }
}

fn from_base64<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s: &str = serde::Deserialize::deserialize(deserializer)?;
    base64::decode(s).map_err(serde::de::Error::custom)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_deserialize() {
        let in_bytes = r#"{"type":"match","data":{"path":{"text":"test_673.txt"},"lines":{"bytes":"dGVzdF9kZGQgICAgLy+ks6TzpMukwaTPoaLApLOmCg=="},"line_number":2,"absolute_offset":9,"submatches":[{"match":{"text":"test_ddd"},"start":0,"end":8}]}}"#;
        let _de = serde_json::from_str::<Message>(in_bytes).unwrap();
    }
}
