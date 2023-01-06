use crate::stdio_server::vim::Vim;
use printer::DisplayLines;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use types::ProgressUpdate;

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
                .unwrap_or_else(|e| panic!("Failed to deserialize preview_size map: {e:?}"));
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

pub struct VimProgressor {
    vim: Vim,
    stopped: Arc<AtomicBool>,
}

impl VimProgressor {
    pub fn new(vim: Vim, stopped: Arc<AtomicBool>) -> Self {
        Self { vim, stopped }
    }
}

impl ProgressUpdate<DisplayLines> for VimProgressor {
    fn update_brief(&self, total_matched: usize, total_processed: usize) {
        if self.stopped.load(Ordering::Relaxed) {
            return;
        }

        let _ = self.vim.exec(
            "clap#state#process_progress",
            json!([total_matched, total_processed]),
        );
    }

    fn update_all(
        &self,
        display_lines: &DisplayLines,
        total_matched: usize,
        total_processed: usize,
    ) {
        if self.stopped.load(Ordering::Relaxed) {
            return;
        }
        let _ = self.vim.exec(
            "clap#state#process_progress_full",
            json!([display_lines, total_matched, total_processed]),
        );
    }

    fn on_finished(
        &self,
        display_lines: DisplayLines,
        total_matched: usize,
        total_processed: usize,
    ) {
        if self.stopped.load(Ordering::Relaxed) {
            return;
        }
        let _ = self.vim.exec(
            "clap#state#process_progress_full",
            json!([display_lines, total_matched, total_processed]),
        );
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
