use anyhow::{anyhow, Result};
use curl::easy::{Easy, List};
use serde::{Deserialize, Serialize};

const USER: &str = "liuchengxu";
const REPO: &str = "vim-clap";

#[derive(Serialize, Deserialize, Debug)]
pub struct RemoteRelease {
    pub tag_name: String,
}

fn retrieve_github_api(api_url: &str) -> Result<Vec<u8>> {
    let mut dst = Vec::new();
    let mut handle = Easy::new();
    handle.url(api_url)?;
    let mut headers = List::new();
    headers.append(&format!("User-Agent: {}", USER))?;
    headers.append("Accept: application/json")?;
    handle.http_headers(headers)?;

    {
        let mut transfer = handle.transfer();
        transfer.write_function(|data| {
            dst.extend_from_slice(data);
            Ok(data.len())
        })?;

        transfer.perform()?;
    }

    Ok(dst)
}

fn retrieve_latest_release() -> Result<Vec<u8>> {
    retrieve_github_api(&format!(
        "https://api.github.com/repos/{}/{}/releases/latest",
        USER, REPO
    ))
}

pub(super) fn retrieve_asset_size(asset_name: &str, tag: &str) -> Result<u64> {
    let data = retrieve_github_api(&format!(
        "https://api.github.com/repos/{}/{}/releases/tags/{}",
        USER, REPO, tag
    ))?;
    let v: serde_json::Value = serde_json::from_slice(&data)?;
    if let serde_json::Value::Array(assets) = &v["assets"] {
        for asset in assets {
            if asset["name"] == asset_name {
                return asset["size"]
                    .as_u64()
                    .ok_or_else(|| anyhow!("Couldn't as u64"));
            }
        }
    }
    Err(anyhow!("Couldn't retrieve size for {}:{}", asset_name, tag))
}

pub fn latest_remote_release() -> Result<RemoteRelease> {
    let data = retrieve_latest_release()?;
    let release: RemoteRelease = serde_json::from_slice(&data)?;
    Ok(release)
}

pub(super) fn get_asset_name() -> Result<String> {
    let asset_name = if cfg!(target_os = "macos") {
        "maple-x86_64-apple-darwin"
    } else if cfg!(target_os = "linux") {
        "maple-x86_64-unknown-linux-musl"
    } else if cfg!(target_os = "windows") {
        "maple-x86_64-pc-windows-msvc"
    } else {
        return Err(anyhow!("no-avaliable-prebuilt-binary for this platform"));
    };
    Ok(asset_name.into())
}

pub(super) fn download_url_for(version: &str) -> Result<String> {
    Ok(format!(
        "https://github.com/{}/{}/releases/download/{}/{}",
        USER,
        REPO,
        version,
        get_asset_name()?
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore]
    fn test_retrieve_asset_size() {
        println!(
            "{:?}",
            retrieve_asset_size(&get_asset_name().unwrap(), "v0.14")
        );
    }
}
