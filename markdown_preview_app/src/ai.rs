//! AI-powered document summarization with configurable providers.
//!
//! Supports Ollama (local), Anthropic, and OpenAI as summary providers.
//! Falls back gracefully when no provider is configured or a request fails.

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

    /// Maximum concurrent summarization requests for this provider.
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

/// AI configuration for summary generation.
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

    /// Whether AI summarization is enabled.
    pub fn is_enabled(&self) -> bool {
        self.provider != AiProvider::None
    }

    /// Resolve API key: config value → env var → error.
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

    /// Resolve Ollama URL: config value → env var → default.
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

/// The system prompt used for summarization.
const SUMMARY_SYSTEM_PROMPT: &str =
    "You are a document summarizer. Provide a concise 2-3 sentence summary of the following \
     markdown document. Focus on the main topic and key points. Do not use markdown formatting \
     in your response. Reply with only the summary, nothing else.";

/// Maximum content length sent to the AI provider (~3000 chars).
const MAX_CONTENT_LEN: usize = 3000;

/// Generate a summary for markdown content.
///
/// Returns `None` if the provider is `None` or the request fails.
pub async fn summarize(config: &AiConfig, content: &str) -> Option<String> {
    if !config.is_enabled() {
        return None;
    }

    let prepared = prepare_content(content);
    if prepared.is_empty() {
        return None;
    }

    let result = match config.provider {
        AiProvider::Ollama => {
            let url = config.effective_ollama_url();
            summarize_ollama(config.model(), &prepared, &url).await
        }
        AiProvider::Anthropic => match config.effective_api_key("ANTHROPIC_API_KEY") {
            Ok(key) => summarize_anthropic(config.model(), &prepared, &key).await,
            Err(error) => {
                tracing::warn!(error = %error, "Missing API key for Anthropic");
                return None;
            }
        },
        AiProvider::OpenAi => match config.effective_api_key("OPENAI_API_KEY") {
            Ok(key) => summarize_openai(config.model(), &prepared, &key).await,
            Err(error) => {
                tracing::warn!(error = %error, "Missing API key for OpenAI");
                return None;
            }
        },
        AiProvider::None => return None,
    };

    match result {
        Ok(summary) => {
            tracing::debug!(
                provider = ?config.provider,
                summary_len = summary.len(),
                "AI summary generated"
            );
            Some(summary)
        }
        Err(error) => {
            tracing::warn!(
                provider = ?config.provider,
                error = %error,
                "AI summarization failed"
            );
            None
        }
    }
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

/// Summarize using a local Ollama instance.
async fn summarize_ollama(model: &str, content: &str, base_url: &str) -> Result<String, String> {
    let url = format!("{base_url}/api/generate");

    let payload = serde_json::json!({
        "model": model,
        "prompt": format!("{SUMMARY_SYSTEM_PROMPT}\n\n{content}"),
        "stream": false
    });

    let response = reqwest::Client::new()
        .post(&url)
        .json(&payload)
        .timeout(std::time::Duration::from_secs(60))
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

/// Summarize using the Anthropic API.
async fn summarize_anthropic(model: &str, content: &str, api_key: &str) -> Result<String, String> {
    let payload = serde_json::json!({
        "model": model,
        "max_tokens": 256,
        "system": SUMMARY_SYSTEM_PROMPT,
        "messages": [
            {"role": "user", "content": content}
        ]
    });

    let response = reqwest::Client::new()
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .json(&payload)
        .timeout(std::time::Duration::from_secs(30))
        .send()
        .await
        .map_err(|e| format!("Anthropic request failed: {e}"))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
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

/// Summarize using the OpenAI API.
async fn summarize_openai(model: &str, content: &str, api_key: &str) -> Result<String, String> {
    let payload = serde_json::json!({
        "model": model,
        "max_tokens": 256,
        "messages": [
            {"role": "system", "content": SUMMARY_SYSTEM_PROMPT},
            {"role": "user", "content": content}
        ]
    });

    let response = reqwest::Client::new()
        .post("https://api.openai.com/v1/chat/completions")
        .header("Authorization", format!("Bearer {api_key}"))
        .header("Content-Type", "application/json")
        .json(&payload)
        .timeout(std::time::Duration::from_secs(30))
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
