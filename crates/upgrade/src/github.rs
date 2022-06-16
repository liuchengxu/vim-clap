use anyhow::Result;
use serde::Deserialize;

const USER: &str = "liuchengxu";
const REPO: &str = "vim-clap";

#[cfg(target_os = "macos")]
pub static PLATFORM: Platform = Platform::MacOS;
#[cfg(target_os = "linux")]
pub static PLATFORM: Platform = Platform::Linux;
#[cfg(target_os = "windows")]
pub static PLATFORM: Platform = Platform::Windows;
#[cfg(all(
    not(target_os = "macos"),
    not(target_os = "linux"),
    not(target_os = "windows")
))]
pub static PLATFORM: Platform = Platform::Unsupported;

pub enum Platform {
    #[cfg(target_os = "macos")]
    MacOS,
    #[cfg(target_os = "linux")]
    Linux,
    #[cfg(target_os = "windows")]
    Windows,
    #[cfg(all(
        not(target_os = "macos"),
        not(target_os = "linux"),
        not(target_os = "windows")
    ))]
    Unsupported,
}

impl Platform {
    pub fn as_asset_name(&self) -> Option<&'static str> {
        match self {
            #[cfg(target_os = "macos")]
            Self::MacOS => Some("maple-x86_64-apple-darwin"),
            #[cfg(target_os = "linux")]
            Self::Linux => Some("maple-x86_64-unknown-linux-musl"),
            #[cfg(target_os = "windows")]
            Self::Windows => Some("maple-x86_64-pc-windows-msvc"),
            #[cfg(all(
                not(target_os = "macos"),
                not(target_os = "linux"),
                not(target_os = "windows")
            ))]
            Self::Unsupported => None,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct Asset {
    pub name: String,
    pub size: u64,
}

// https://docs.github.com/en/rest/releases/releases
#[derive(Debug, Deserialize)]
pub struct Release {
    pub tag_name: String,
    pub assets: Vec<Asset>,
}

pub(super) async fn retrieve_asset_size(asset_name: &str, tag: &str) -> Result<u64> {
    let url = format!("https://api.github.com/repos/{USER}/{REPO}/releases/tags/{tag}",);

    let res = reqwest::Client::new()
        .get(url)
        .header("Accept", "application/vnd.github.v3+json")
        .header("User-Agent", USER)
        .send()
        .await?;

    let release = res.json::<Release>().await?;

    release
        .assets
        .iter()
        .find(|x| x.name == asset_name)
        .map(|x| x.size)
        .ok_or_else(|| panic!("Can not find the asset {asset_name} in given release {tag}"))
}

pub(super) async fn retrieve_latest_release() -> Result<Release> {
    let url = format!("https://api.github.com/repos/{USER}/{REPO}/releases/latest",);

    let release = reqwest::Client::new()
        .get(url)
        .header("Accept", "application/vnd.github.v3+json")
        .header("User-Agent", USER)
        .send()
        .await?
        .json::<Release>()
        .await?;

    Ok(release)
}

pub(super) fn download_url(version: &str) -> Option<String> {
    PLATFORM.as_asset_name().map(|asset_name| {
        format!("https://github.com/{USER}/{REPO}/releases/download/{version}/{asset_name}",)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_retrieve_asset_size() {
        let asset_name = PLATFORM.as_asset_name().unwrap();
        retrieve_asset_size(asset_name, "v0.34")
            .await
            .expect("Failed to retrieve the asset size for release v0.34");
    }
}
