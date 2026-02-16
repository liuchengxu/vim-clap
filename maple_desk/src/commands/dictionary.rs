//! Dictionary lookup commands: AI-powered and offline (StarDict/MDict).

use std::sync::Arc;

use serde::Serialize;
use tauri::State;
use tokio::sync::RwLock;

use crate::ai::{self, AiConfig, DictionaryEntry};
use crate::state::{AppState, DictLoadResult, OfflineDictState};

/// Type alias for the managed dictionary load serializer.
type DictLoadMutex = Arc<tokio::sync::Mutex<()>>;

/// A single offline lookup result from one dictionary.
#[derive(Serialize)]
pub struct OfflineLookupResult {
    pub dict_name: String,
    pub word: String,
    pub html: String,
}

/// Information about a loaded dictionary.
#[derive(Serialize)]
pub struct DictionaryInfo {
    pub name: String,
    pub word_count: usize,
}

/// Response from an AI dictionary lookup, indicating whether it was served from cache.
#[derive(Serialize)]
pub struct DictionaryLookupResponse {
    /// The dictionary entry.
    #[serde(flatten)]
    pub entry: DictionaryEntry,
    /// Whether this result came from the cache (no AI request made).
    pub cached: bool,
}

/// Look up a word in the AI-powered dictionary.
#[tauri::command]
pub async fn lookup_word(
    word: String,
    state: State<'_, Arc<RwLock<AppState>>>,
) -> Result<DictionaryLookupResponse, String> {
    let word = word.trim().to_string();
    if word.is_empty() {
        return Err("No word provided".to_string());
    }

    // Check cache first (read lock)
    {
        let state_guard = state.read().await;
        if let Some(cached) = state_guard.get_cached_dict_entry(&word) {
            tracing::debug!(word = %word, "Dictionary cache hit");
            return Ok(DictionaryLookupResponse {
                entry: cached.clone(),
                cached: true,
            });
        }
    }

    // Cache miss — build config and call AI
    let config = {
        let state_guard = state.read().await;
        AiConfig::from_state(
            state_guard.ai_provider(),
            state_guard.ai_model(),
            state_guard.ai_api_key(),
            state_guard.ollama_url(),
        )
    };

    if !config.is_enabled() {
        return Err("AI provider not configured. Set one in Settings (gear icon).".to_string());
    }

    tracing::debug!(word = %word, "Dictionary cache miss — calling AI");
    let entry = ai::lookup_word(&config, &word).await?;

    // Store in cache (write lock)
    {
        let mut state_guard = state.write().await;
        state_guard.set_cached_dict_entry(word, entry.clone());
    }

    Ok(DictionaryLookupResponse {
        entry,
        cached: false,
    })
}

/// Ask a free-form question to the AI.
#[tauri::command]
pub async fn ask_ai(
    question: String,
    state: State<'_, Arc<RwLock<AppState>>>,
) -> Result<String, String> {
    let question = question.trim().to_string();
    if question.is_empty() {
        return Err("No question provided".to_string());
    }

    let config = {
        let state_guard = state.read().await;
        AiConfig::from_state(
            state_guard.ai_provider(),
            state_guard.ai_model(),
            state_guard.ai_api_key(),
            state_guard.ollama_url(),
        )
    };

    if !config.is_enabled() {
        return Err("AI provider not configured. Set one in Settings (gear icon).".to_string());
    }

    ai::ask_ai(&config, &question).await
}

/// Look up a word in all loaded offline dictionaries.
#[tauri::command]
pub async fn lookup_word_offline(
    word: String,
    dict_state: State<'_, Arc<RwLock<OfflineDictState>>>,
    load_mutex: State<'_, DictLoadMutex>,
) -> Result<Vec<OfflineLookupResult>, String> {
    let word = word.trim().to_string();
    if word.is_empty() {
        return Err("No word provided".to_string());
    }

    ensure_dicts_loaded(&dict_state, &load_mutex).await?;

    let guard = dict_state.read().await;
    let mut results = Vec::new();
    for dict in guard.dicts() {
        if let Some(hit) = dict.lookup(&word) {
            results.push(OfflineLookupResult {
                dict_name: dict.name().to_string(),
                word: word.clone(),
                html: hit.html,
            });
        }
    }
    Ok(results)
}

/// Get info about all loaded dictionaries.
#[tauri::command]
pub async fn get_loaded_dictionaries(
    dict_state: State<'_, Arc<RwLock<OfflineDictState>>>,
    load_mutex: State<'_, DictLoadMutex>,
) -> Result<Vec<DictionaryInfo>, String> {
    ensure_dicts_loaded(&dict_state, &load_mutex).await?;

    let guard = dict_state.read().await;
    let infos = guard
        .dicts()
        .iter()
        .map(|d| DictionaryInfo {
            name: d.name().to_string(),
            word_count: d.word_count(),
        })
        .collect();
    Ok(infos)
}

/// Ensure dictionaries are loaded for the current dirs.
///
/// `OfflineDictState` is the sole source of truth for target dirs — no reads from
/// `AppState`. `set_app_config` pushes dir changes into `OfflineDictState` via
/// `update_dirs()` (which bumps `dirs_version` and sets `loaded=false`) before
/// persisting to `AppState`, so there is no window where a stale `is_loaded()=true`
/// can be observed against new config.
///
/// Serialized via `load_mutex`. All file I/O runs in `spawn_blocking`.
/// Stale loads (dirs changed during load) are discarded via `dirs_version` check.
async fn ensure_dicts_loaded(
    dict_state: &Arc<RwLock<OfflineDictState>>,
    load_mutex: &tokio::sync::Mutex<()>,
) -> Result<(), String> {
    // Fast path: already loaded for the current version
    {
        let guard = dict_state.read().await;
        if guard.is_loaded() {
            return Ok(());
        }
    }

    // Serialize loads to prevent thundering herd
    let _lock = load_mutex.lock().await;

    // Re-check after acquiring mutex (another task may have loaded while we waited).
    // Also snapshot dirs + version atomically from the single source of truth,
    // and check whether a recent total-failure is still in cooldown.
    let (dirs, version) = {
        let guard = dict_state.read().await;
        if guard.is_loaded() {
            return Ok(());
        }
        let (dirs, version) = guard.dirs_snapshot();
        if guard.is_in_failure_cooldown(version) {
            tracing::debug!("Skipping dictionary load — within failure cooldown");
            return Ok(());
        }
        (dirs, version)
    };

    if dirs.is_empty() {
        // No dirs configured — commit empty dicts for this version
        let mut guard = dict_state.write().await;
        guard.commit_load(Vec::new(), version);
        return Ok(());
    }

    // Load on blocking thread (file I/O + decompression)
    let result: DictLoadResult = tokio::task::spawn_blocking(move || {
        OfflineDictState::load_dicts_from_dirs(&dirs)
    })
    .await
    .map_err(|error| format!("Dictionary loading task failed: {error}"))?;

    // All candidates failed: record failure with cooldown instead of marking loaded.
    // Retries are suppressed for LOAD_FAILURE_COOLDOWN_SECS, then the next lookup
    // tries again. Changing dirs via settings clears the cooldown immediately.
    if result.dicts.is_empty() && result.candidates_found > 0 {
        tracing::warn!(
            candidates = result.candidates_found,
            "All dictionary files failed to load — will retry after cooldown"
        );
        let mut guard = dict_state.write().await;
        guard.record_load_failure(version);
        return Ok(());
    }

    // Commit if version still matches (includes legitimate 0-candidate case)
    let mut guard = dict_state.write().await;
    if !guard.commit_load(result.dicts, version) {
        tracing::info!("Dictionary dirs changed during load — discarding stale results");
    }

    Ok(())
}
