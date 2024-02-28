use serde::de::DeserializeOwned;
use serde::Deserialize;

const USER: &str = "liuchengxu";
const REPO: &str = "vim-clap";

pub(super) fn maple_asset_name() -> Option<&'static str> {
    if cfg!(target_os = "macos") {
        if cfg!(target_arch = "x86_64") {
            Some("maple-x86_64-apple-darwin")
        } else if cfg!(target_arch = "aarch64") {
            Some("maple-aarch64-apple-darwin")
        } else {
            None
        }
    } else if cfg!(target_os = "linux") {
        if cfg!(target_arch = "x86_64") {
            Some("maple-x86_64-unknown-linux-musl")
        } else if cfg!(target_arch = "aarch64") {
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

pub(super) fn maple_asset_download_url(version: &str) -> Option<String> {
    maple_asset_name().map(|asset_name| {
        format!("https://github.com/{USER}/{REPO}/releases/download/{version}/{asset_name}",)
    })
}

#[derive(Debug, Deserialize)]
pub struct Asset {
    pub name: String,
    pub size: u64,
    pub browser_download_url: String,
}

// https://docs.github.com/en/rest/releases/releases
#[derive(Debug, Deserialize)]
pub struct GitHubRelease {
    pub tag_name: String,
    pub assets: Vec<Asset>,
}

async fn request<T: DeserializeOwned>(url: &str) -> std::io::Result<T> {
    let io_error =
        |e| std::io::Error::new(std::io::ErrorKind::Other, format!("Reqwest error: {e}"));

    reqwest::Client::new()
        .get(url)
        .header("Accept", "application/vnd.github.v3+json")
        .header("User-Agent", USER)
        .send()
        .await
        .map_err(io_error)?
        .json::<T>()
        .await
        .map_err(io_error)
}

pub async fn latest_github_release(user: &str, repo: &str) -> std::io::Result<GitHubRelease> {
    let url = format!("https://api.github.com/repos/{user}/{repo}/releases/latest");
    request::<GitHubRelease>(&url).await
}

pub(super) async fn latest_vim_clap_github_release() -> std::io::Result<GitHubRelease> {
    latest_github_release(USER, REPO).await
}

pub(super) async fn fetch_maple_asset_size(asset_name: &str, tag: &str) -> std::io::Result<u64> {
    let url = format!("https://api.github.com/repos/{USER}/{REPO}/releases/tags/{tag}");
    let release: GitHubRelease = request(&url).await?;

    release
        .assets
        .iter()
        .find(|x| x.name == asset_name)
        .map(|x| x.size)
        .ok_or_else(|| panic!("Can not find the asset {asset_name} in given release {tag}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_retrieve_asset_size() {
        if crate::tests::is_commit_associated_with_a_tag() {
            return;
        }

        for _i in 0..20 {
            if let Ok(latest_tag) = latest_vim_clap_github_release().await.map(|r| r.tag_name) {
                fetch_maple_asset_size(maple_asset_name().unwrap(), &latest_tag)
                    .await
                    .expect("Failed to retrieve the asset size for latest release");
                return;
            }
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        }

        panic!("Failed to retrieve the asset size for latest release");
    }
}
