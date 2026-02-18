//! AI-powered features with configurable providers.
//!
//! Supports Ollama (local), Anthropic, and OpenAI as AI providers.
//! Provides generic request helpers used by summarization, dictionary lookup,
//! and future tools.

use serde::{Deserialize, Serialize};

/// Supported AI providers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AiProvider {
    /// No AI provider configured.
    None,
    /// Local Ollama instance.
    Ollama,
    /// Anthropic API (Claude).
    Anthropic,
    /// OpenAI API.
    OpenAi,
}

impl AiProvider {
    /// Parse a provider name string into an `AiProvider`.
    pub fn from_name(name: &str) -> Self {
        match name.to_lowercase().as_str() {
            "ollama" => Self::Ollama,
            "anthropic" => Self::Anthropic,
            "openai" => Self::OpenAi,
            _ => Self::None,
        }
    }

    /// Maximum concurrent requests for this provider.
    pub fn max_concurrency(&self) -> usize {
        match self {
            // Ollama runs locally — limit to 1 to avoid resource contention
            Self::Ollama => 1,
            // Cloud APIs can handle parallel requests
            Self::Anthropic | Self::OpenAi => 4,
            Self::None => 1,
        }
    }

    /// Default model for this provider.
    fn default_model(&self) -> &str {
        match self {
            Self::Ollama => "llama3.2",
            Self::Anthropic => "claude-sonnet-4-5-20250929",
            Self::OpenAi => "gpt-4o-mini",
            Self::None => "",
        }
    }
}

/// AI configuration for provider access.
pub struct AiConfig {
    /// Which provider to use.
    pub provider: AiProvider,
    /// Model override (uses provider default if None).
    pub model: Option<String>,
    /// API key from config (for Anthropic/OpenAI).
    pub api_key: Option<String>,
    /// Ollama URL from config.
    pub ollama_url: Option<String>,
}

impl AiConfig {
    /// Create config from persisted state values.
    pub fn from_state(
        provider: Option<&str>,
        model: Option<&str>,
        api_key: Option<&str>,
        ollama_url: Option<&str>,
    ) -> Self {
        let provider = provider
            .map(AiProvider::from_name)
            .unwrap_or(AiProvider::None);
        Self {
            provider,
            model: model.map(String::from),
            api_key: api_key.map(String::from),
            ollama_url: ollama_url.map(String::from),
        }
    }

    /// Get the effective model name.
    fn model(&self) -> &str {
        self.model
            .as_deref()
            .unwrap_or_else(|| self.provider.default_model())
    }

    /// Whether AI is enabled.
    pub fn is_enabled(&self) -> bool {
        self.provider != AiProvider::None
    }

    /// Resolve API key: config value -> env var -> error.
    fn effective_api_key(&self, env_var: &str) -> Result<String, String> {
        self.api_key
            .as_deref()
            .map(str::trim)
            .filter(|k| !k.is_empty())
            .map(String::from)
            .or_else(|| {
                std::env::var(env_var)
                    .ok()
                    .map(|s| s.trim().to_string())
                    .filter(|k| !k.is_empty())
            })
            .ok_or_else(|| format!("{env_var} not configured (set in Settings or environment)"))
    }

    /// Resolve Ollama URL: config value -> env var -> default.
    fn effective_ollama_url(&self) -> String {
        self.ollama_url
            .as_deref()
            .map(str::trim)
            .filter(|u| !u.is_empty())
            .map(String::from)
            .or_else(|| {
                std::env::var("OLLAMA_URL")
                    .ok()
                    .map(|s| s.trim().to_string())
                    .filter(|u| !u.is_empty())
            })
            .unwrap_or_else(|| "http://localhost:11434".to_string())
    }
}

// ============================================================================
// Generic per-provider request helpers
// ============================================================================

/// Send a request to a local Ollama instance.
async fn ollama_request(
    model: &str,
    system_prompt: &str,
    user_content: &str,
    base_url: &str,
    timeout_secs: u64,
) -> Result<String, String> {
    let url = format!("{base_url}/api/generate");

    let payload = serde_json::json!({
        "model": model,
        "system": system_prompt,
        "prompt": user_content,
        "stream": false
    });

    let response = reqwest::Client::new()
        .post(&url)
        .json(&payload)
        .timeout(std::time::Duration::from_secs(timeout_secs))
        .send()
        .await
        .map_err(|e| format!("Ollama request failed: {e}"))?;

    if !response.status().is_success() {
        return Err(format!("Ollama returned status {}", response.status()));
    }

    let body: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse Ollama response: {e}"))?;

    body["response"]
        .as_str()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "Empty response from Ollama".to_string())
}

/// Whether the given key is a Claude OAuth setup token (vs a regular API key).
fn is_oauth_token(key: &str) -> bool {
    key.contains("sk-ant-oat")
}

/// Send a request to the Anthropic API.
///
/// Automatically detects whether `api_key` is a regular API key or an OAuth setup
/// token (from `claude setup-token`) and adjusts auth headers and system prompt
/// accordingly. OAuth tokens require Claude Code identity headers to be accepted.
async fn anthropic_request(
    model: &str,
    system_prompt: &str,
    user_content: &str,
    api_key: &str,
    max_tokens: u32,
    timeout_secs: u64,
) -> Result<String, String> {
    let oauth = is_oauth_token(api_key);

    // OAuth tokens require the Claude Code identity in the system prompt.
    let effective_system = if oauth {
        format!(
            "You are Claude Code, Anthropic's official CLI for Claude.\n\n{system_prompt}"
        )
    } else {
        system_prompt.to_string()
    };

    let payload = serde_json::json!({
        "model": model,
        "max_tokens": max_tokens,
        "system": effective_system,
        "messages": [
            {"role": "user", "content": user_content}
        ]
    });

    let mut request = reqwest::Client::new()
        .post("https://api.anthropic.com/v1/messages")
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json");

    if oauth {
        // OAuth tokens use Bearer auth and must present Claude Code identity headers.
        request = request
            .header("Authorization", format!("Bearer {api_key}"))
            .header(
                "anthropic-beta",
                "claude-code-20250219,oauth-2025-04-20",
            )
            .header("user-agent", "claude-cli/2.1.2 (external, cli)")
            .header("x-app", "cli");
    } else {
        request = request.header("x-api-key", api_key);
    }

    let response = request
        .json(&payload)
        .timeout(std::time::Duration::from_secs(timeout_secs))
        .send()
        .await
        .map_err(|e| format!("Anthropic request failed: {e}"))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        if oauth && status == reqwest::StatusCode::UNAUTHORIZED {
            return Err(
                "Anthropic OAuth token expired or invalid. \
                 Run `claude setup-token` to generate a new one and paste it in Settings."
                    .to_string(),
            );
        }
        return Err(format!("Anthropic returned status {status}: {body}"));
    }

    let body: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse Anthropic response: {e}"))?;

    body["content"][0]["text"]
        .as_str()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "Empty response from Anthropic".to_string())
}

/// Send a request to the OpenAI API.
async fn openai_request(
    model: &str,
    system_prompt: &str,
    user_content: &str,
    api_key: &str,
    max_tokens: u32,
    timeout_secs: u64,
) -> Result<String, String> {
    let payload = serde_json::json!({
        "model": model,
        "max_tokens": max_tokens,
        "messages": [
            {"role": "system", "content": system_prompt},
            {"role": "user", "content": user_content}
        ]
    });

    let response = reqwest::Client::new()
        .post("https://api.openai.com/v1/chat/completions")
        .header("Authorization", format!("Bearer {api_key}"))
        .header("Content-Type", "application/json")
        .json(&payload)
        .timeout(std::time::Duration::from_secs(timeout_secs))
        .send()
        .await
        .map_err(|e| format!("OpenAI request failed: {e}"))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("OpenAI returned status {status}: {body}"));
    }

    let body: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse OpenAI response: {e}"))?;

    body["choices"][0]["message"]["content"]
        .as_str()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "Empty response from OpenAI".to_string())
}

/// Send an AI request using the configured provider.
///
/// This is the primary entry point for any tool that needs AI text generation.
/// Dispatches to the correct provider based on config, handling API key resolution.
pub async fn ai_request(
    config: &AiConfig,
    system_prompt: &str,
    user_content: &str,
    max_tokens: u32,
) -> Result<String, String> {
    if !config.is_enabled() {
        return Err("AI provider not configured".to_string());
    }

    match config.provider {
        AiProvider::Ollama => {
            let url = config.effective_ollama_url();
            ollama_request(config.model(), system_prompt, user_content, &url, 60).await
        }
        AiProvider::Anthropic => {
            let key = config.effective_api_key("ANTHROPIC_API_KEY")?;
            anthropic_request(
                config.model(),
                system_prompt,
                user_content,
                &key,
                max_tokens,
                30,
            )
            .await
        }
        AiProvider::OpenAi => {
            let key = config.effective_api_key("OPENAI_API_KEY")?;
            openai_request(
                config.model(),
                system_prompt,
                user_content,
                &key,
                max_tokens,
                30,
            )
            .await
        }
        AiProvider::None => Err("AI provider not configured".to_string()),
    }
}

// ============================================================================
// Summarization
// ============================================================================

/// The system prompt used for summarization.
const SUMMARY_SYSTEM_PROMPT: &str =
    "You are a document summarizer. Provide a concise 2-3 sentence summary of the following \
     markdown document. Focus on the main topic and key points. Do not use markdown formatting \
     in your response. Reply with only the summary, nothing else.";

/// Maximum content length sent to the AI provider (~3000 chars).
const MAX_CONTENT_LEN: usize = 3000;

/// Generate a summary for markdown content.
///
/// Returns `Ok(summary)` on success, `Err(message)` on failure.
pub async fn summarize(config: &AiConfig, content: &str) -> Result<String, String> {
    let prepared = prepare_content(content);
    if prepared.is_empty() {
        return Err("No content to summarize".to_string());
    }

    let result = ai_request(config, SUMMARY_SYSTEM_PROMPT, &prepared, 256).await;

    match &result {
        Ok(summary) => {
            tracing::debug!(
                provider = ?config.provider,
                summary_len = summary.len(),
                "AI summary generated"
            );
        }
        Err(error) => {
            tracing::warn!(
                provider = ?config.provider,
                error = %error,
                "AI summarization failed"
            );
        }
    }

    result
}

/// Strip frontmatter, code blocks, and truncate content for the AI prompt.
fn prepare_content(content: &str) -> String {
    let mut result = String::new();
    let mut in_frontmatter = false;
    let mut frontmatter_delimiters = 0;
    let mut in_code_block = false;

    for line in content.lines() {
        let trimmed = line.trim();

        // Handle YAML frontmatter
        if trimmed == "---" {
            frontmatter_delimiters += 1;
            if frontmatter_delimiters == 1 {
                in_frontmatter = true;
                continue;
            } else if frontmatter_delimiters == 2 {
                in_frontmatter = false;
                continue;
            }
        }

        if in_frontmatter {
            continue;
        }

        // Handle code blocks
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            in_code_block = !in_code_block;
            continue;
        }

        if in_code_block {
            continue;
        }

        result.push_str(line);
        result.push('\n');

        if result.len() >= MAX_CONTENT_LEN {
            result.truncate(MAX_CONTENT_LEN);
            break;
        }
    }

    result
}

// ============================================================================
// Dictionary
// ============================================================================

/// The system prompt for dictionary lookups.
const DICTIONARY_SYSTEM_PROMPT: &str = "\
You are an English dictionary. Given a word, return a JSON object with this exact structure:
{
  \"word\": \"the word\",
  \"phonetic\": \"IPA pronunciation\",
  \"definitions\": [
    {
      \"part_of_speech\": \"noun/verb/adjective/etc\",
      \"meaning\": \"definition text\",
      \"meaning_zh\": \"中文释义\",
      \"example\": \"example sentence using the word\"
    }
  ],
  \"synonyms\": [\"word1\", \"word2\"],
  \"antonyms\": [\"word1\", \"word2\"],
  \"etymology\": \"Brief origin: language roots, morphemes, historical evolution (1-2 sentences)\",
  \"mnemonic\": \"A short, vivid memory tip to help remember the word (1 sentence)\",
  \"word_family\": [
    {\"word\": \"derived_form\", \"part_of_speech\": \"noun\"}
  ],
  \"related_concepts\": [\"concept1\", \"concept2\"]
}
Include multiple definitions if the word has different parts of speech or meanings. \
meaning_zh: a concise Chinese translation/explanation of each definition to aid understanding. \
Provide 2-5 synonyms and antonyms when applicable (empty arrays if none). \
etymology: concise origin with language roots (Latin, Greek, etc.) and key morphemes. \
mnemonic: a vivid, memorable tip (association, visual image, or wordplay). \
word_family: 3-6 derived/inflected forms (e.g. for \"happy\": happiness, unhappy, happily). \
related_concepts: 3-6 thematically related words or concepts for a semantic map. \
Reply with ONLY the JSON object, no other text.";

/// A member of a word family (derived/related form).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WordFamilyMember {
    /// The word form.
    pub word: String,
    /// Part of speech of this form.
    #[serde(default)]
    pub part_of_speech: String,
}

/// A single definition entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DictionaryDefinition {
    /// Part of speech (noun, verb, adjective, etc.)
    pub part_of_speech: String,
    /// The definition text.
    pub meaning: String,
    /// Chinese translation/explanation of the definition.
    #[serde(default)]
    pub meaning_zh: String,
    /// An example sentence.
    #[serde(default)]
    pub example: String,
}

/// A complete dictionary entry for a word.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DictionaryEntry {
    /// The looked-up word.
    pub word: String,
    /// IPA phonetic transcription.
    #[serde(default)]
    pub phonetic: String,
    /// List of definitions.
    pub definitions: Vec<DictionaryDefinition>,
    /// Synonyms.
    #[serde(default)]
    pub synonyms: Vec<String>,
    /// Antonyms.
    #[serde(default)]
    pub antonyms: Vec<String>,
    /// Brief etymology / word origin.
    #[serde(default)]
    pub etymology: String,
    /// Mnemonic tip for memorization.
    #[serde(default)]
    pub mnemonic: String,
    /// Word family: derived and related forms.
    #[serde(default)]
    pub word_family: Vec<WordFamilyMember>,
    /// Related concepts for a semantic map.
    #[serde(default)]
    pub related_concepts: Vec<String>,
}

/// Look up a word using the configured AI provider.
///
/// Returns a structured dictionary entry parsed from the AI's JSON response.
pub async fn lookup_word(config: &AiConfig, word: &str) -> Result<DictionaryEntry, String> {
    let raw = ai_request(config, DICTIONARY_SYSTEM_PROMPT, word, 1536).await?;

    // Strip markdown code fences if the model wrapped the JSON
    let json_str = strip_code_fences(&raw);

    serde_json::from_str::<DictionaryEntry>(json_str)
        .map_err(|e| format!("Failed to parse dictionary response: {e}"))
}

// ============================================================================
// Ask AI (free-form Q&A)
// ============================================================================

/// The system prompt for the Ask AI tool.
const ASK_AI_SYSTEM_PROMPT: &str =
    "You are a helpful language assistant. You specialize in English language usage, \
     grammar, expressions, idioms, and phrasing, but you can answer general questions too. \
     Give clear, concise answers. Use examples when helpful. \
     You may use markdown formatting for structure.";

/// Send a free-form question to the AI and return the answer as text.
pub async fn ask_ai(config: &AiConfig, question: &str) -> Result<String, String> {
    ai_request(config, ASK_AI_SYSTEM_PROMPT, question, 1024).await
}

/// Strip optional markdown code fences (```json ... ```) from AI output.
fn strip_code_fences(text: &str) -> &str {
    let trimmed = text.trim();
    if let Some(rest) = trimmed.strip_prefix("```") {
        // Skip optional language tag on the first line
        let rest = rest.find('\n').map(|idx| &rest[idx + 1..]).unwrap_or(rest);
        rest.strip_suffix("```").unwrap_or(rest).trim()
    } else {
        trimmed
    }
}
