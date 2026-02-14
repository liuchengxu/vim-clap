//! URL fetching and rendering commands.

use super::RenderResponse;
use markdown_preview_core::{calculate_document_stats, to_html, RenderOptions};

/// Parsed GitHub URL components.
struct GitHubUrl {
    owner: String,
    repo: String,
    git_ref: String,
    path: String,
}

/// Parse a GitHub blob URL into its components.
/// Example: https://github.com/owner/repo/blob/branch/path/to/file.md
fn parse_github_url(url: &str) -> Option<GitHubUrl> {
    let url = url.strip_prefix("https://github.com/")?;

    // Split by /blob/
    let (repo_part, path_part) = url.split_once("/blob/")?;

    // repo_part = "owner/repo"
    let (owner, repo) = repo_part.split_once('/')?;

    // path_part = "branch/path/to/file.md" or "commit_sha/path/to/file.md"
    let (git_ref, path) = path_part.split_once('/')?;

    Some(GitHubUrl {
        owner: owner.to_string(),
        repo: repo.to_string(),
        git_ref: git_ref.to_string(),
        path: path.to_string(),
    })
}

/// Convert a GitHub URL to raw content URL (for public repos).
fn convert_to_raw_github_url(url: &str) -> Option<String> {
    // Already a raw URL
    if url.starts_with("https://raw.githubusercontent.com/") {
        return Some(url.to_string());
    }

    // Convert blob URL to raw URL
    if url.starts_with("https://github.com/") && url.contains("/blob/") {
        let raw_url = url
            .replace("https://github.com/", "https://raw.githubusercontent.com/")
            .replace("/blob/", "/");
        return Some(raw_url);
    }

    None
}

/// Get GitHub token from environment.
fn get_github_token() -> Option<String> {
    std::env::var("GITHUB_TOKEN")
        .or_else(|_| std::env::var("GH_TOKEN"))
        .ok()
        .filter(|t| !t.is_empty())
}

/// Fetch content from GitHub API (works for private repos with token).
async fn fetch_github_api(github_url: &GitHubUrl, token: &str) -> Result<String, String> {
    // GitHub API endpoint for file contents
    let api_url = format!(
        "https://api.github.com/repos/{}/{}/contents/{}?ref={}",
        github_url.owner, github_url.repo, github_url.path, github_url.git_ref
    );

    tracing::info!(api_url = %api_url, "Fetching from GitHub API");

    let client = reqwest::Client::new();
    let response = client
        .get(&api_url)
        .header("Authorization", format!("Bearer {token}"))
        .header("Accept", "application/vnd.github.raw+json")
        .header("User-Agent", "markdown-preview-app")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .send()
        .await
        .map_err(|e| format!("Failed to fetch from GitHub API: {e}"))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("GitHub API error: HTTP {status} - {body}"));
    }

    response
        .text()
        .await
        .map_err(|e| format!("Failed to read GitHub API response: {e}"))
}

/// Fetch and render markdown from a URL.
#[tauri::command]
pub async fn open_url(url: String) -> Result<RenderResponse, String> {
    open_url_impl(&url, get_github_token().as_deref()).await
}

/// Fetch and render markdown from a URL with a user-provided GitHub token.
///
/// This is used when the user provides a token via the UI after initial fetch fails.
#[tauri::command]
pub async fn open_url_with_token(url: String, token: String) -> Result<RenderResponse, String> {
    let token = if token.trim().is_empty() {
        None
    } else {
        Some(token)
    };
    open_url_impl(&url, token.as_deref()).await
}

/// Internal implementation for URL fetching with optional token.
async fn open_url_impl(url: &str, token: Option<&str>) -> Result<RenderResponse, String> {
    tracing::info!(url = %url, has_token = token.is_some(), "Opening URL");

    let content = if let Some(github_url) = parse_github_url(url) {
        // It's a GitHub URL - try API first if we have a token
        if let Some(token) = token {
            tracing::info!("Using GitHub API with token");
            match fetch_github_api(&github_url, token).await {
                Ok(content) => content,
                Err(e) => {
                    tracing::warn!(error = %e, "GitHub API failed, trying raw URL");
                    // Fall back to raw URL
                    fetch_raw_url(url).await?
                }
            }
        } else {
            // No token, use raw URL (public repos only)
            fetch_raw_url(url).await?
        }
    } else {
        // Not a GitHub URL, fetch directly
        fetch_raw_url(url).await?
    };

    // Render the markdown
    let result = to_html(&content, &RenderOptions::gui()).map_err(|e| e.to_string())?;
    let stats = calculate_document_stats(&content);

    Ok(RenderResponse::from_markdown(result, stats).with_file_info(Some(url.to_string()), None))
}

/// Fetch content from a URL (with GitHub raw URL conversion).
async fn fetch_raw_url(url: &str) -> Result<String, String> {
    let fetch_url = convert_to_raw_github_url(url).unwrap_or_else(|| url.to_string());

    tracing::info!(fetch_url = %fetch_url, "Fetching raw URL");

    let response = reqwest::get(&fetch_url)
        .await
        .map_err(|e| format!("Failed to fetch URL: {e}"))?;

    if !response.status().is_success() {
        let status = response.status();
        let status_code = status.as_u16();
        // 404 on GitHub could mean private repo, 401/403 means auth required
        if url.contains("github.com")
            && (status_code == 404 || status_code == 401 || status_code == 403)
        {
            // Use AUTH_REQUIRED: prefix so frontend can detect and prompt for token
            return Err(format!(
                "AUTH_REQUIRED:HTTP {status}: This may be a private repository. Would you like to provide a GitHub token to access it?"
            ));
        }
        return Err(format!("Failed to fetch URL: HTTP {status}"));
    }

    response
        .text()
        .await
        .map_err(|e| format!("Failed to read response: {e}"))
}
