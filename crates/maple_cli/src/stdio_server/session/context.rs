use std::path::PathBuf;
use std::sync::{atomic::AtomicBool, Arc};

use anyhow::Result;
use icon::IconPainter;
use matcher::MatchType;
use parking_lot::Mutex;
use serde::Deserialize;

use crate::stdio_server::{types::ProviderId, Message};

const DEFAULT_DISPLAY_WINWIDTH: u64 = 100;

const DEFAULT_PREVIEW_WINHEIGHT: u64 = 30;

/// This type represents the scale of filtering source.
#[derive(Debug, Clone)]
pub enum Scale {
    /// We do not know the exact total number of source items.
    Indefinite,

    /// Large scale.
    ///
    /// The number of total source items is already known, but that's
    /// too many for the synchorous filtering.
    Large(usize),

    /// Small scale, in which case we do not have to use the dynamic filtering.
    Small { total: usize, lines: Vec<String> },
}

impl Default for Scale {
    fn default() -> Self {
        Self::Indefinite
    }
}

impl Scale {
    pub fn total(&self) -> Option<usize> {
        match self {
            Self::Large(total) | Self::Small { total, .. } => Some(*total),
            _ => None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct SyncFilterResults {
    pub total: usize,
    pub lines: Vec<String>,
    pub indices: Vec<Vec<usize>>,
    pub truncated_map: printer::LinesTruncatedMap,
}

#[derive(Clone, Debug)]
pub enum Icon {
    Disabled,
    Enabled(IconPainter),
}

impl From<Icon> for Option<IconPainter> {
    fn from(icon: Icon) -> Self {
        match icon {
            Icon::Disabled => None,
            Icon::Enabled(icon) => Some(icon),
        }
    }
}

#[derive(Debug, Clone)]
pub struct SessionContext {
    pub provider_id: ProviderId,
    pub cwd: PathBuf,
    pub start_buffer_path: PathBuf,
    pub display_winwidth: u64,
    pub preview_winheight: u64,
    pub icon: Icon,
    pub match_type: MatchType,
    pub scale: Arc<Mutex<Scale>>,
    pub source_cmd: Option<String>,
    pub runtimepath: Option<String>,
    pub is_running: Arc<Mutex<AtomicBool>>,
}

impl SessionContext {
    /// Executes the command `cmd` and returns the raw bytes of stdout.
    pub fn execute(&self, cmd: &str) -> Result<Vec<u8>> {
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

    pub fn sync_filter_source_item<'a>(
        &self,
        query: &str,
        lines: impl Iterator<Item = &'a str>,
    ) -> Result<SyncFilterResults> {
        let ranked = filter::sync_run(
            query,
            filter::Source::List(lines.map(Into::into)), // TODO: optimize as_str().into(), clone happens there.
            matcher::FuzzyAlgorithm::Fzy,
            self.match_type.clone(),
            Vec::new(),
        )?;

        let total = ranked.len();

        // Take the first 200 entries and add an icon to each of them.
        let (lines, indices, truncated_map) = printer::process_top_items(
            ranked.iter().take(200).cloned().collect(),
            self.display_winwidth as usize,
            self.icon.clone().into(),
        );

        Ok(SyncFilterResults {
            total,
            lines,
            indices,
            truncated_map,
        })
    }

    // TODO: optimize as_str().into(), clone happens there.
    pub fn sync_filter_full_line<'a>(
        &self,
        query: &'a str,
        lines: impl Iterator<Item = &'a str>,
    ) -> Result<SyncFilterResults> {
        let fuzzy_matcher = matcher::Matcher::with_bonuses(
            matcher::FuzzyAlgorithm::Fzy,
            self.match_type.clone(),
            Vec::new(),
        );
        let ranked = filter::sync_run_on_small_scale(query, lines, fuzzy_matcher)?;

        let total = ranked.len();

        // Take the first 200 entries and add an icon to each of them.
        let (lines, indices, truncated_map) = printer::process_top_items(
            ranked.iter().take(200).cloned().collect(),
            self.display_winwidth as usize,
            self.icon.clone().into(),
        );

        Ok(SyncFilterResults {
            total,
            lines,
            indices,
            truncated_map,
        })
    }
}

impl From<Message> for SessionContext {
    fn from(msg: Message) -> Self {
        log::debug!("Creating a new SessionContext from: {:?}", msg);

        #[derive(Deserialize)]
        struct Params {
            provider_id: ProviderId,
            cwd: PathBuf,
            source_fpath: PathBuf,
            display_winwidth: Option<u64>,
            preview_winheight: Option<u64>,
            source_cmd: Option<String>,
            runtimepath: Option<String>,
            enable_icon: Option<bool>,
        }

        let Params {
            provider_id,
            cwd,
            source_fpath,
            display_winwidth,
            preview_winheight,
            source_cmd,
            runtimepath,
            enable_icon,
        } = msg.deserialize_params_unsafe();

        let match_type = match provider_id.as_str() {
            "tags" | "proj_tags" => MatchType::TagName,
            "grep" | "grep2" => MatchType::IgnoreFilePath,
            _ => MatchType::Full,
        };

        let icon = if enable_icon.unwrap_or(false) {
            match provider_id.as_str() {
                "proj_tags" => Icon::Enabled(IconPainter::ProjTags),
                "grep" | "grep2" => Icon::Enabled(IconPainter::Grep),
                _ => Icon::Disabled,
            }
        } else {
            Icon::Disabled
        };

        Self {
            provider_id,
            cwd,
            start_buffer_path: source_fpath,
            display_winwidth: display_winwidth.unwrap_or(DEFAULT_DISPLAY_WINWIDTH),
            preview_winheight: preview_winheight.unwrap_or(DEFAULT_PREVIEW_WINHEIGHT),
            source_cmd,
            runtimepath,
            match_type,
            icon,
            scale: Arc::new(Mutex::new(Scale::Indefinite)),
            is_running: Arc::new(Mutex::new(true.into())),
        }
    }
}
