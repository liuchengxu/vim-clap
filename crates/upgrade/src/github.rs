use anyhow::{anyhow, Result};
use serde::Deserialize;

const USER: &str = "liuchengxu";
const REPO: &str = "vim-clap";

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

pub(super) fn asset_name() -> Result<&'static str> {
    let asset_name = if cfg!(target_os = "macos") {
        "maple-x86_64-apple-darwin"
    } else if cfg!(target_os = "linux") {
        "maple-x86_64-unknown-linux-musl"
    } else if cfg!(target_os = "windows") {
        "maple-x86_64-pc-windows-msvc"
    } else {
        return Err(anyhow!("no-avaliable-prebuilt-binary for this platform"));
    };
    Ok(asset_name)
}

pub(super) fn download_url(version: &str) -> Result<String> {
    Ok(format!(
        "https://github.com/{}/{}/releases/download/{}/{}",
        USER,
        REPO,
        version,
        asset_name()?
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_retrieve_asset_size() {
        retrieve_asset_size(&asset_name().unwrap(), "v0.34")
            .await
            .expect("Failed to retrieve the asset size for release v0.34");
    }
}
