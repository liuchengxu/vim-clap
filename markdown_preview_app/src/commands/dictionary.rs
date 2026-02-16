//! Dictionary lookup command using the configured AI provider.

use std::sync::Arc;

use tauri::State;
use tokio::sync::RwLock;

use crate::ai::{self, AiConfig, DictionaryEntry};
use crate::state::AppState;

/// Look up a word in the AI-powered dictionary.
#[tauri::command]
pub async fn lookup_word(
    word: String,
    state: State<'_, Arc<RwLock<AppState>>>,
) -> Result<DictionaryEntry, String> {
    let word = word.trim().to_string();
    if word.is_empty() {
        return Err("No word provided".to_string());
    }

    let config = {
        let state = state.read().await;
        AiConfig::from_state(
            state.ai_provider(),
            state.ai_model(),
            state.ai_api_key(),
            state.ollama_url(),
        )
    };

    if !config.is_enabled() {
        return Err("AI provider not configured. Set one in Settings (gear icon).".to_string());
    }

    tracing::debug!(word = %word, "Looking up word in dictionary");

    ai::lookup_word(&config, &word).await
}
