use std::path::PathBuf;
use std::sync::{atomic::AtomicBool, Arc};

use parking_lot::Mutex;
use serde::Deserialize;

use icon::Icon;
use matcher::{ClapItem, Matcher};
use types::MatchedItem;

use crate::stdio_server::rpc::{Call, MethodCall, Notification, Params};
use crate::stdio_server::types::ProviderId;

const DEFAULT_DISPLAY_WINWIDTH: usize = 100;

const DEFAULT_PREVIEW_WINHEIGHT: usize = 30;

/// This type represents the scale of filtering source.
#[derive(Debug, Clone)]
pub enum SourceScale {
    /// We do not know the exact total number of source items.
    Unknown,

    /// Large scale.
    ///
    /// The number of total source items is already known, but that's
    /// too many for the synchorous filtering.
    Large(usize),

    // TODO: Use Arc<dyn ClapItem> instead of String.
    /// Small scale, in which case we do not have to use the dynamic filtering.
    Small {
        total: usize,
        items: Vec<Arc<dyn ClapItem>>,
    },

    /// Unknown scale, but the cache exists.
    Cache { total: usize, path: PathBuf },
}

impl Default for SourceScale {
    fn default() -> Self {
        Self::Unknown
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
            Self::Small { ref items, .. } => Some(
                items
                    .iter()
                    .take(n)
                    .map(|item| {
                        MatchedItem::new(item.clone(), Default::default(), Default::default())
                    })
                    .collect(),
            ),
            Self::Cache { ref path, .. } => utility::read_first_lines(path, n)
                .map(|iter| {
                    iter.map(|line| {
                        MatchedItem::new(Arc::new(line), Default::default(), Default::default())
                    })
                    .collect()
                })
                .ok(),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SessionState {
    pub is_running: Arc<AtomicBool>,
    pub source_scale: Arc<Mutex<SourceScale>>,
}

/// bufnr and winid.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BufnrAndWinid {
    pub bufnr: u64,
    pub winid: u64,
}

#[derive(Debug, Clone)]
pub struct SessionContext {
    pub provider_id: ProviderId,
    pub start: BufnrAndWinid,
    pub input: BufnrAndWinid,
    pub display: BufnrAndWinid,
    pub cwd: PathBuf,
    pub no_cache: bool,
    pub debounce: bool,
    pub start_buffer_path: PathBuf,
    pub display_winwidth: usize,
    pub preview_winheight: usize,
    pub icon: Icon,
    pub matcher: Matcher,
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

    pub fn set_source_scale(&self, new: SourceScale) {
        let mut source_scale = self.state.source_scale.lock();
        *source_scale = new;
    }

    fn from_params(params: Params) -> Self {
        #[derive(Deserialize)]
        struct InnerParams {
            provider_id: ProviderId,
            start: BufnrAndWinid,
            input: BufnrAndWinid,
            display: BufnrAndWinid,
            cwd: PathBuf,
            no_cache: bool,
            debounce: Option<bool>,
            source_fpath: PathBuf,
            display_winwidth: Option<usize>,
            preview_winheight: Option<usize>,
            source_cmd: Option<String>,
            runtimepath: Option<String>,
            enable_icon: Option<bool>,
        }

        let InnerParams {
            provider_id,
            start,
            input,
            display,
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

        let icon = if enable_icon.unwrap_or(false) {
            provider_id.icon()
        } else {
            Icon::Null
        };

        let matcher = provider_id.matcher();

        Self {
            input,
            display,
            start,
            provider_id,
            cwd,
            no_cache,
            debounce: debounce.unwrap_or(true),
            start_buffer_path: source_fpath,
            display_winwidth: display_winwidth.unwrap_or(DEFAULT_DISPLAY_WINWIDTH),
            preview_winheight: preview_winheight.unwrap_or(DEFAULT_PREVIEW_WINHEIGHT),
            source_cmd,
            runtimepath,
            matcher,
            icon,
            state: SessionState {
                is_running: Arc::new(true.into()),
                source_scale: Arc::new(Mutex::new(SourceScale::Unknown)),
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
        tracing::debug!(?call, "Creating a new SessionContext");
        match call {
            Call::MethodCall(method_call) => method_call.into(),
            Call::Notification(notification) => notification.into(),
        }
    }
}
