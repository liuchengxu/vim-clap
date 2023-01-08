use serde::{de::DeserializeOwned, Deserialize};

const USER: &str = "liuchengxu";
const REPO: &str = "vim-clap";

pub(super) fn asset_name() -> Option<&'static str> {
    if cfg!(target_os = "macos") {
        if cfg!(target_arch = "x86_64-apple-darwin") {
            Some("maple-x86_64-apple-darwin")
        } else if cfg!(target_arch = "aarch64-apple-darwin") {
            Some("maple-aarch64-apple-darwin")
        } else {
            None
        }
    } else if cfg!(target_os = "linux") {
        if cfg!(target_arch = "x86_64-unknown-linux-musl") {
            Some("maple-x86_64-unknown-linux-musl")
        } else if cfg!(target_arch = "x86_64-unknown-linux-gnu") {
            Some("maple-x86_64-unknown-linux-gnu")
        } else if cfg!(target_arch = "aarch64-unknown-linux-gnu") {
            Some("maple-aarch64-unknown-linux-gnu")
        } else {
            None
        }
    } else if cfg!(target_os = "windows") {
        Some("maple-x86_64-pc-windows-msvc")
    } else {
        None
    }
}

pub(super) fn asset_download_url(version: &str) -> Option<String> {
    asset_name().map(|asset_name| {
        format!("https://github.com/{USER}/{REPO}/releases/download/{version}/{asset_name}",)
    })
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

async fn request<T: DeserializeOwned>(url: &str) -> std::io::Result<T> {
    let to_io_error =
        |e| std::io::Error::new(std::io::ErrorKind::Other, format!("Reqwest error: {e}"));

    reqwest::Client::new()
        .get(url)
        .header("Accept", "application/vnd.github.v3+json")
        .header("User-Agent", USER)
        .send()
        .await
        .map_err(to_io_error)?
        .json::<T>()
        .await
        .map_err(to_io_error)
}

pub(super) async fn retrieve_asset_size(asset_name: &str, tag: &str) -> std::io::Result<u64> {
    let url = format!("https://api.github.com/repos/{USER}/{REPO}/releases/tags/{tag}",);
    let release: Release = request(&url).await?;

    release
        .assets
        .iter()
        .find(|x| x.name == asset_name)
        .map(|x| x.size)
        .ok_or_else(|| panic!("Can not find the asset {asset_name} in given release {tag}"))
}

pub(super) async fn retrieve_latest_release() -> std::io::Result<Release> {
    let url = format!("https://api.github.com/repos/{USER}/{REPO}/releases/latest",);
    request::<Release>(&url).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_retrieve_asset_size() {
        let latest_tag = retrieve_latest_release().await.unwrap().tag_name;
        retrieve_asset_size(asset_name().unwrap(), &latest_tag)
            .await
            .expect("Failed to retrieve the asset size for release v0.34");
    }
}
