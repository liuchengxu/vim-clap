use indicatif::{ProgressBar, ProgressStyle};
use serde::de::DeserializeOwned;
use serde::Deserialize;
use std::path::PathBuf;
use tokio::io::AsyncWriteExt;

#[derive(Debug, Deserialize)]
pub struct Asset {
    pub name: String,
    pub size: u64,
    #[allow(dead_code)]
    pub browser_download_url: String,
}

// https://docs.github.com/en/rest/releases/releases
#[derive(Debug, Deserialize)]
pub struct GitHubRelease {
    pub tag_name: String,
    pub assets: Vec<Asset>,
}

pub async fn request<T: DeserializeOwned>(url: &str, user_agent: &str) -> std::io::Result<T> {
    let io_error =
        |e| std::io::Error::new(std::io::ErrorKind::Other, format!("Reqwest error: {e}"));

    reqwest::Client::new()
        .get(url)
        .header("Accept", "application/vnd.github.v3+json")
        .header("User-Agent", user_agent)
        .send()
        .await
        .map_err(io_error)?
        .json::<T>()
        .await
        .map_err(io_error)
}

pub async fn latest_github_release(user: &str, repo: &str) -> std::io::Result<GitHubRelease> {
    let url = format!("https://api.github.com/repos/{user}/{repo}/releases/latest");
    request::<GitHubRelease>(&url, user).await
}

pub enum DownloadResult {
    /// File already exists in the specified path.
    Existed(PathBuf),
    /// File was downloaded successfully to the given path.
    Success(PathBuf),
}

/// Download an asset file from GitHub to the local file system.
pub async fn download_asset_file(
    version: &str,
    asset_name: &str,
    total_size: u64,
    asset_download_url: &str,
    no_progress_bar: bool,
) -> std::io::Result<DownloadResult> {
    let mut tmp = std::env::temp_dir();
    tmp.push(format!("{version}-{asset_name}"));

    // Check if there is a partially downloaded binary before.
    if tmp.is_file() {
        let metadata = std::fs::metadata(&tmp)?;
        if metadata.len() == total_size {
            return Ok(DownloadResult::Existed(tmp));
        } else {
            std::fs::remove_file(&tmp)?;
        }
    }

    let mut maybe_progress_bar = if no_progress_bar {
        None
    } else {
        let progress_bar = ProgressBar::new(total_size);
        progress_bar.set_style(ProgressStyle::default_bar()
                       .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})")
                       .progress_chars("#>-"));
        Some(progress_bar)
    };

    let to_io_error =
        |e| std::io::Error::new(std::io::ErrorKind::Other, format!("Reqwest error: {e}"));

    let mut source = reqwest::Client::new()
        .get(asset_download_url)
        .send()
        .await
        .map_err(to_io_error)?;

    let mut dest = tokio::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&tmp.as_path())
        .await?;

    while let Some(chunk) = source.chunk().await.map_err(to_io_error)? {
        dest.write_all(&chunk).await?;

        if let Some(ref mut progress_bar) = maybe_progress_bar {
            progress_bar.inc(chunk.len() as u64);
        }
    }

    Ok(DownloadResult::Success(tmp))
}
