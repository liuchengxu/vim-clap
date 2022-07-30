use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{atomic::AtomicBool, Arc};

use anyhow::Result;
use parking_lot::Mutex;
use serde::Deserialize;

use icon::{Icon, IconKind};
use matcher::MatchScope;
use types::MatchedItem;

use crate::command::ctags::buffer_tags::BufferTagInfo;
use crate::stdio_server::rpc::{Call, MethodCall, Notification, Params};
use crate::stdio_server::types::ProviderId;

const DEFAULT_DISPLAY_WINWIDTH: u64 = 100;

const DEFAULT_PREVIEW_WINHEIGHT: u64 = 30;

/// This type represents the scale of filtering source.
#[derive(Debug, Clone)]
pub enum SourceScale {
    /// We do not know the exact total number of source items.
    Indefinite,

    /// Large scale.
    ///
    /// The number of total source items is already known, but that's
    /// too many for the synchorous filtering.
    Large(usize),

    // TODO: Use Arc<dyn ClapItem> instead of String.
    /// Small scale, in which case we do not have to use the dynamic filtering.
    Small { total: usize, lines: Vec<String> },

    /// Unknown scale, but the cache exists.
    Cache { total: usize, path: PathBuf },
}

impl Default for SourceScale {
    fn default() -> Self {
        Self::Indefinite
    }
}

impl SourceScale {
    pub fn total(&self) -> Option<usize> {
        match self {
            Self::Large(total) | Self::Small { total, .. } | Self::Cache { total, .. } => {
                Some(*total)
            }
            _ => None,
        }
    }

    pub fn initial_lines(&self, n: usize) -> Option<Vec<MatchedItem>> {
        match self {
            Self::Small { ref lines, .. } => Some(
                lines
                    .iter()
                    .take(n)
                    .map(|s| {
                        MatchedItem::new(
                            Arc::new(s.to_string()),
                            Default::default(),
                            Default::default(),
                        )
                    })
                    .collect(),
            ),
            Self::Cache { ref path, .. } => {
                if let Ok(lines_iter) = utility::read_first_lines(path, n) {
                    Some(
                        lines_iter
                            .map(|line| {
                                MatchedItem::new(
                                    Arc::new(line),
                                    Default::default(),
                                    Default::default(),
                                )
                            })
                            .collect::<Vec<_>>(),
                    )
                } else {
                    None
                }
            }
            _ => None,
        }
    }
}

// TODO: cache the buffer tags per session.
#[derive(Debug, Clone)]
pub struct CachedBufTags {
    pub done: bool,
    pub tags: Vec<BufferTagInfo>,
}

#[derive(Debug, Clone)]
pub struct SessionState {
    pub is_running: Arc<AtomicBool>,
    pub source_scale: Arc<Mutex<SourceScale>>,
    pub buf_tags_cache: Arc<Mutex<HashMap<PathBuf, CachedBufTags>>>,
}

#[derive(Debug, Clone)]
pub struct SessionContext {
    pub provider_id: ProviderId,
    pub cwd: PathBuf,
    pub no_cache: bool,
    pub debounce: bool,
    pub start_buffer_path: PathBuf,
    pub display_winwidth: u64,
    pub preview_winheight: u64,
    pub icon: Icon,
    pub match_scope: MatchScope,
    pub match_bonuses: Vec<matcher::Bonus>,
    pub source_cmd: Option<String>,
    pub runtimepath: Option<String>,
    pub state: SessionState,
}

impl SessionContext {
    /// Executes the command `cmd` and returns the raw bytes of stdout.
    pub fn execute(&self, cmd: &str) -> std::io::Result<Vec<u8>> {
        let out = utility::execute_at(cmd, Some(&self.cwd))?;
        Ok(out.stdout)
    }

    /// Size for fulfilling the preview window.
    pub fn sensible_preview_size(&self) -> usize {
        std::cmp::max(
            self.provider_id.get_preview_size(),
            (self.preview_winheight / 2) as usize,
        )
    }

    pub fn fuzzy_matcher(&self) -> matcher::Matcher {
        matcher::Matcher::with_bonuses(
            Vec::new(), // TODO: bonuses
            matcher::FuzzyAlgorithm::Fzy,
            self.match_scope,
        )
    }

    pub fn set_source_scale(&self, new: SourceScale) {
        let mut source_scale = self.state.source_scale.lock();
        *source_scale = new;
    }

    fn from_params(params: Params) -> Self {
        #[derive(Deserialize)]
        struct InnerParams {
            provider_id: ProviderId,
            cwd: PathBuf,
            no_cache: bool,
            debounce: Option<bool>,
            source_fpath: PathBuf,
            display_winwidth: Option<u64>,
            preview_winheight: Option<u64>,
            source_cmd: Option<String>,
            runtimepath: Option<String>,
            enable_icon: Option<bool>,
        }

        let InnerParams {
            provider_id,
            cwd,
            no_cache,
            debounce,
            source_fpath,
            display_winwidth,
            preview_winheight,
            source_cmd,
            runtimepath,
            enable_icon,
        } = params
            .parse()
            .expect("Failed to deserialize SessionContext");

        let match_scope = match provider_id.as_str() {
            "tags" | "proj_tags" => MatchScope::TagName,
            "grep" | "grep2" => MatchScope::GrepLine,
            _ => MatchScope::Full,
        };

        let icon = if enable_icon.unwrap_or(false) {
            match provider_id.as_str() {
                "tags" => Icon::Enabled(IconKind::BufferTags),
                "proj_tags" => Icon::Enabled(IconKind::ProjTags),
                "grep" | "grep2" => Icon::Enabled(IconKind::Grep),
                "files" => Icon::Enabled(IconKind::File),
                _ => Icon::Null,
            }
        } else {
            Icon::Null
        };

        let match_bonuses = match provider_id.as_str() {
            "files" | "git_files" | "filer" => vec![matcher::Bonus::FileName],
            _ => vec![],
        };

        Self {
            provider_id,
            cwd,
            no_cache,
            debounce: debounce.unwrap_or(true),
            start_buffer_path: source_fpath,
            display_winwidth: display_winwidth.unwrap_or(DEFAULT_DISPLAY_WINWIDTH),
            preview_winheight: preview_winheight.unwrap_or(DEFAULT_PREVIEW_WINHEIGHT),
            source_cmd,
            runtimepath,
            match_scope,
            match_bonuses,
            icon,
            state: SessionState {
                is_running: Arc::new(true.into()),
                source_scale: Arc::new(Mutex::new(SourceScale::Indefinite)),
                buf_tags_cache: Arc::new(Mutex::new(HashMap::new())),
            },
        }
    }
}

impl From<MethodCall> for SessionContext {
    fn from(method_call: MethodCall) -> Self {
        Self::from_params(method_call.params)
    }
}

impl From<Notification> for SessionContext {
    fn from(notification: Notification) -> Self {
        Self::from_params(notification.params)
    }
}

impl From<Call> for SessionContext {
    fn from(call: Call) -> Self {
        tracing::debug!(?call, "Creating a new SessionContext from given call");
        match call {
            Call::MethodCall(method_call) => method_call.into(),
            Call::Notification(notification) => notification.into(),
        }
    }
}
