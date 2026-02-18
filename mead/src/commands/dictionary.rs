//! Dictionary lookup commands: AI-powered, online (Free Dictionary API),
//! and offline (StarDict/MDict).

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tauri::State;
use tokio::sync::RwLock;

use crate::ai::{self, AiConfig, DictionaryDefinition, DictionaryEntry};
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

// ---------------------------------------------------------------------------
// Wiktionary etymology lookup
// ---------------------------------------------------------------------------

/// Result from a Wiktionary etymology lookup.
#[derive(Serialize)]
pub struct EtymologyResult {
    /// Raw HTML of the etymology section (sanitized on the frontend via DOMPurify).
    pub etymology_html: String,
    /// Source label (always "Wiktionary").
    pub source: String,
}

/// Top-level response from the Wiktionary parse API (sections query).
#[derive(Deserialize)]
struct WikiSectionsResponse {
    #[serde(default)]
    parse: Option<WikiParseSections>,
}

/// The `parse` object containing sections.
#[derive(Deserialize)]
struct WikiParseSections {
    #[serde(default)]
    sections: Vec<WikiSection>,
}

/// A single section from the Wiktionary parse API.
#[derive(Deserialize)]
struct WikiSection {
    #[serde(default)]
    line: String,
    #[serde(default)]
    index: String,
}

/// Top-level response from the Wiktionary parse API (text query).
#[derive(Deserialize)]
struct WikiTextResponse {
    #[serde(default)]
    parse: Option<WikiParseText>,
}

/// The `parse` object containing rendered text.
#[derive(Deserialize)]
struct WikiParseText {
    #[serde(default)]
    text: std::collections::HashMap<String, String>,
}

/// Strip MediaWiki wrapper markup from the etymology section HTML.
///
/// Removes the outer `<div class="mw-parser-output">`, heading tags, and HTML comments.
fn clean_wiktionary_html(raw: &str) -> String {
    let mut result = raw.to_string();

    // Remove outer wrapper div
    if let Some(start) = result.find("<div class=\"mw-parser-output\">") {
        let end_tag = "</div>";
        result = result[start + "<div class=\"mw-parser-output\">".len()..].to_string();
        if let Some(last_div) = result.rfind(end_tag) {
            result.truncate(last_div);
        }
    }

    // Remove heading tags (h1-h6)
    let heading_re =
        regex::Regex::new(r"(?i)<h[1-6][^>]*>.*?</h[1-6]>").expect("valid heading regex");
    result = heading_re.replace_all(&result, "").to_string();

    // Remove HTML comments
    let comment_re = regex::Regex::new(r"<!--.*?-->").expect("valid comment regex");
    result = comment_re.replace_all(&result, "").to_string();

    result.trim().to_string()
}

/// Look up the etymology of a word from Wiktionary.
///
/// Uses the MediaWiki parse API in two steps:
/// 1. Fetch section list and find the first "Etymology" section.
/// 2. Fetch the rendered HTML of that section.
///
/// Returns `Ok(None)` if the word or etymology section is not found.
#[tauri::command]
pub async fn lookup_etymology(word: String) -> Result<Option<EtymologyResult>, String> {
    let word = word.trim().to_string();
    if word.is_empty() {
        return Err("No word provided".to_string());
    }

    let client = reqwest::Client::builder()
        .user_agent("MEAD/1.0")
        .timeout(std::time::Duration::from_secs(8))
        .build()
        .map_err(|error| format!("HTTP client error: {error}"))?;

    // Step 1: Get sections list
    let sections_resp = client
        .get("https://en.wiktionary.org/w/api.php")
        .query(&[
            ("action", "parse"),
            ("page", &word),
            ("prop", "sections"),
            ("format", "json"),
        ])
        .send()
        .await
        .map_err(|error| format!("Wiktionary sections request failed: {error}"))?;

    if !sections_resp.status().is_success() {
        return Ok(None);
    }

    let sections_body: WikiSectionsResponse = sections_resp
        .json()
        .await
        .map_err(|error| format!("Failed to parse Wiktionary sections response: {error}"))?;

    let sections = match sections_body.parse {
        Some(p) => p.sections,
        None => return Ok(None),
    };

    // Find the first Etymology section
    let etym_index = sections
        .iter()
        .find(|s| s.line.starts_with("Etymology"))
        .map(|s| s.index.clone());

    let section_index = match etym_index {
        Some(idx) => idx,
        None => return Ok(None),
    };

    // Step 2: Fetch the etymology section text
    let text_resp = client
        .get("https://en.wiktionary.org/w/api.php")
        .query(&[
            ("action", "parse"),
            ("page", &word),
            ("prop", "text"),
            ("section", &section_index),
            ("format", "json"),
        ])
        .send()
        .await
        .map_err(|error| format!("Wiktionary text request failed: {error}"))?;

    if !text_resp.status().is_success() {
        return Ok(None);
    }

    let text_body: WikiTextResponse = text_resp
        .json()
        .await
        .map_err(|error| format!("Failed to parse Wiktionary text response: {error}"))?;

    let raw_html = match text_body.parse {
        Some(p) => p.text.get("*").cloned().unwrap_or_default(),
        None => return Ok(None),
    };

    if raw_html.is_empty() {
        return Ok(None);
    }

    let cleaned = clean_wiktionary_html(&raw_html);
    if cleaned.is_empty() {
        return Ok(None);
    }

    Ok(Some(EtymologyResult {
        etymology_html: cleaned,
        source: "Wiktionary".to_string(),
    }))
}

// ---------------------------------------------------------------------------
// Free Dictionary API (dictionaryapi.dev) — online lookup
// ---------------------------------------------------------------------------

/// Response from an online dictionary lookup, pairing the structured entry
/// with the original source name (e.g. "Wiktionary").
#[derive(Serialize)]
pub struct OnlineLookupResult {
    /// The dictionary entry (same shape as AI lookups).
    #[serde(flatten)]
    pub entry: DictionaryEntry,
    /// Human-readable source name extracted from `sourceUrls`.
    pub source: String,
}

/// Top-level entry from the Free Dictionary API response.
#[derive(Deserialize)]
struct FreeDictEntry {
    phonetic: Option<String>,
    #[serde(default)]
    meanings: Vec<FreeDictMeaning>,
    #[serde(default, rename = "sourceUrls")]
    source_urls: Vec<String>,
}

/// A meaning group (one per part of speech).
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FreeDictMeaning {
    part_of_speech: String,
    #[serde(default)]
    definitions: Vec<FreeDictDefinition>,
    #[serde(default)]
    synonyms: Vec<String>,
    #[serde(default)]
    antonyms: Vec<String>,
}

/// A single definition inside a meaning group.
#[derive(Deserialize)]
struct FreeDictDefinition {
    definition: String,
    example: Option<String>,
    #[serde(default)]
    synonyms: Vec<String>,
    #[serde(default)]
    antonyms: Vec<String>,
}

/// Convert Free Dictionary API entries into a [`DictionaryEntry`].
///
/// Merges all entries into a single result, collecting synonyms/antonyms from
/// both the meaning-group and per-definition levels.
fn free_dict_to_entry(word: String, raw: &[FreeDictEntry]) -> DictionaryEntry {
    let phonetic = raw
        .iter()
        .find_map(|e| e.phonetic.clone())
        .unwrap_or_default();

    let mut definitions = Vec::new();
    let mut synonyms = Vec::new();
    let mut antonyms = Vec::new();

    for entry in raw {
        for meaning in &entry.meanings {
            // Collect top-level synonyms/antonyms from the meaning group.
            synonyms.extend(meaning.synonyms.iter().cloned());
            antonyms.extend(meaning.antonyms.iter().cloned());

            for def in &meaning.definitions {
                definitions.push(DictionaryDefinition {
                    part_of_speech: meaning.part_of_speech.clone(),
                    meaning: def.definition.clone(),
                    meaning_zh: String::new(),
                    example: def.example.clone().unwrap_or_default(),
                });
                synonyms.extend(def.synonyms.iter().cloned());
                antonyms.extend(def.antonyms.iter().cloned());
            }
        }
    }

    // De-duplicate while preserving order.
    synonyms.dedup();
    antonyms.dedup();

    DictionaryEntry {
        word,
        phonetic,
        definitions,
        synonyms,
        antonyms,
        etymology: String::new(),
        mnemonic: String::new(),
        word_family: Vec::new(),
        related_concepts: Vec::new(),
    }
}

/// Extract a human-readable source name from a URL.
///
/// For example, `https://en.wiktionary.org/wiki/hello` → `"Wiktionary"`.
/// Falls back to the domain segment if the name isn't recognised.
fn source_name_from_url(url: &str) -> Option<String> {
    // Extract host from URL: skip "https://" or "http://", take up to next '/'.
    let after_scheme = url.split("://").nth(1)?;
    let host = after_scheme.split('/').next()?;

    // Strip leading "en." / "www." etc.
    let base = host
        .strip_prefix("en.")
        .or_else(|| host.strip_prefix("www."))
        .unwrap_or(host);

    // Capitalise the first segment before the first dot.
    let name = base.split('.').next().unwrap_or(base);
    let mut chars = name.chars();
    let capitalised: String = match chars.next() {
        Some(c) => c.to_uppercase().chain(chars).collect(),
        None => return None,
    };
    Some(capitalised)
}

/// Look up a word using the Free Dictionary API (dictionaryapi.dev).
///
/// Returns an [`OnlineLookupResult`] containing the structured entry and the
/// original source name (e.g. "Wiktionary").
/// Returns `None` when the word is not found (HTTP 404).
#[tauri::command]
pub async fn lookup_word_online(word: String) -> Result<Option<OnlineLookupResult>, String> {
    let word = word.trim().to_string();
    if word.is_empty() {
        return Err("No word provided".to_string());
    }

    let url = format!("https://api.dictionaryapi.dev/api/v2/entries/en/{word}");

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|error| format!("HTTP client error: {error}"))?;

    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|error| format!("Network error: {error}"))?;

    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(None);
    }

    if !response.status().is_success() {
        return Err(format!(
            "Free Dictionary API returned status {}",
            response.status()
        ));
    }

    let entries: Vec<FreeDictEntry> = response
        .json()
        .await
        .map_err(|error| format!("Failed to parse Free Dictionary response: {error}"))?;

    if entries.is_empty() {
        return Ok(None);
    }

    let source = entries
        .iter()
        .flat_map(|e| e.source_urls.iter())
        .find_map(|u| source_name_from_url(u))
        .unwrap_or_else(|| "Free Dictionary".to_string());

    Ok(Some(OnlineLookupResult {
        entry: free_dict_to_entry(word, &entries),
        source,
    }))
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
    let result: DictLoadResult =
        tokio::task::spawn_blocking(move || OfflineDictState::load_dicts_from_dirs(&dirs))
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
