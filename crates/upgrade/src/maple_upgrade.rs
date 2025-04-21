use crate::github::{
    download_asset_file, latest_github_release, request, DownloadResult, GitHubRelease,
};
use std::path::PathBuf;

fn asset_name() -> Option<&'static str> {
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

fn maple_asset_download_url(version: &str) -> Option<String> {
    asset_name().map(|asset_name| {
        format!("https://github.com/liuchengxu/vim-clap/releases/download/{version}/{asset_name}",)
    })
}

async fn fetch_asset_size(asset_name: &str, tag: &str) -> std::io::Result<u64> {
    let url = format!("https://api.github.com/repos/liuchengxu/vim-clap/releases/tags/{tag}");
    let release: GitHubRelease = request(&url, "liuchengxu").await?;

    release
        .assets
        .iter()
        .find(|x| x.name == asset_name)
        .map(|x| x.size)
        .ok_or_else(|| panic!("Can not find the asset {asset_name} in given release {tag}"))
}

/// This command is only invoked when user uses the prebuilt binary, more specifically, the
/// executable runs from `vim-clap/bin/maple`.
#[derive(Debug, Clone)]
pub struct Upgrade {
    /// Download if the local version mismatches the latest remote version.
    pub download: bool,
    /// Disable the downloading progress_bar
    pub no_progress_bar: bool,
}

impl Upgrade {
    pub fn new(download: bool, no_progress_bar: bool) -> Self {
        Self {
            download,
            no_progress_bar,
        }
    }

    pub async fn run(&self, local_tag: &str) -> std::io::Result<()> {
        println!("Retrieving the latest remote release info...");
        let latest_release = latest_github_release("liuchengxu", "vim-clap").await?;
        let latest_tag = latest_release.tag_name;
        let latest_version = extract_remote_version_number(&latest_tag);
        let local_version = extract_local_version_number(local_tag);

        if latest_version != local_version {
            if self.download {
                println!("New maple release {latest_tag} is available, downloading...",);

                let temp_file = download_prebuilt_binary(&latest_tag, self.no_progress_bar).await?;

                // Only tries to upgrade if using the prebuilt binary, i.e., `bin/maple`.
                let bin_path = get_binary_path()?;

                // Move the downloaded binary to bin/maple
                std::fs::rename(temp_file, bin_path)?;

                println!("Latest version {latest_tag} download completed");
            } else {
                match maple_asset_download_url(&latest_tag) {
                    Some(url) => {
                        println!("New maple release {latest_tag} is available, please download it from {url} or rerun with --download flag.");
                    }
                    None => {
                        println!("New maple release {latest_tag} is available, but no prebuilt binary provided for your platform");
                    }
                }
            }
        } else {
            println!("No newer prebuilt binary release, current maple version: {local_version}");
        }

        Ok(())
    }
}

/// The prebuilt binary is put at bin/maple.
fn get_binary_path() -> std::io::Result<impl AsRef<std::path::Path>> {
    use std::io::{Error, ErrorKind};

    let exe_dir = std::env::current_exe()?;
    let bin_dir = exe_dir.parent().ok_or_else(|| {
        Error::new(
            ErrorKind::NotFound,
            "Parent directory of current executable not found",
        )
    })?;

    if !bin_dir.ends_with("bin") {
        return Err(Error::other("Current executable is not from bin/***"));
    }

    let bin_path = if cfg!(windows) {
        bin_dir.join("maple.exe")
    } else {
        bin_dir.join("maple")
    };

    Ok(bin_path)
}

/// Extracts the number of version from tag name, e.g., returns 13 out of the tag `v0.13`.
#[inline]
fn extract_remote_version_number(remote_tag: &str) -> u32 {
    remote_tag
        .split('.')
        .nth(1)
        .and_then(|s| s.parse().ok())
        .expect("Couldn't extract remote version")
}

/// local: "v0.13-4-g58738c0"
#[inline]
fn extract_local_version_number(local_tag: &str) -> u32 {
    let tag = local_tag.split('-').next().expect("Invalid local tag");
    extract_remote_version_number(tag)
}

#[cfg(unix)]
fn set_executable_permission<P: AsRef<std::path::Path>>(path: P) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mut perms = std::fs::metadata(path.as_ref())?.permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(path.as_ref(), perms)?;
    Ok(())
}

/// Downloads the latest remote release binary to a temp file.
///
/// # Arguments
///
/// - `version`: "v0.13"
async fn download_prebuilt_binary(
    version: &str,
    no_progress_bar: bool,
) -> std::io::Result<PathBuf> {
    let binary_unavailable =
        || std::io::Error::other("No available prebuilt binary for this platform");

    let asset_name = asset_name().ok_or_else(binary_unavailable)?;
    let total_size = fetch_asset_size(asset_name, version).await?;
    let download_url = maple_asset_download_url(version).ok_or_else(binary_unavailable)?;

    let tmp = match download_asset_file(
        version,
        asset_name,
        total_size,
        &download_url,
        no_progress_bar,
    )
    .await?
    {
        DownloadResult::Existed(tmp) => {
            println!("{} has already been downloaded", tmp.display());
            tmp
        }
        DownloadResult::Success(tmp) => {
            println!("Download of '{}' has been completed.", tmp.display());
            tmp
        }
    };

    #[cfg(unix)]
    set_executable_permission(&tmp)?;

    Ok(tmp)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn is_commit_associated_with_a_tag() -> bool {
        let output = std::process::Command::new("git")
            .args(["rev-parse", "HEAD"])
            .output()
            .expect("Failed to find HEAD commit");
        let commit_id = String::from_utf8_lossy(&output.stdout);

        std::process::Command::new("git")
            .args(["describe", "--tags", "--exact-match", commit_id.trim()])
            .status()
            .map(|exit_status| exit_status.success())
            .unwrap_or(false)
    }

    #[test]
    fn test_extract_version_number() {
        let tag = "v0.13-4-g58738c0";
        assert_eq!(13u32, extract_local_version_number(tag));
        let tag = "v0.13";
        assert_eq!(13u32, extract_local_version_number(tag));
    }

    #[tokio::test]
    async fn test_retrieve_asset_size() {
        if is_commit_associated_with_a_tag() {
            return;
        }

        for _i in 0..20 {
            if let Ok(latest_tag) = latest_github_release("liuchengxu", "vim-clap")
                .await
                .map(|r| r.tag_name)
            {
                fetch_asset_size(asset_name().unwrap(), &latest_tag)
                    .await
                    .expect("Failed to retrieve the asset size for latest release");
                return;
            }
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        }

        panic!("Failed to retrieve the asset size for latest release");
    }

    #[tokio::test]
    async fn test_download_prebuilt_binary() {
        // Ignore this test when the commit is associated with a tag as the binary is possibly not
        // yet able to be uploaded to the release page.
        if is_commit_associated_with_a_tag() {
            return;
        }

        for _i in 0..20 {
            if let Ok(latest_tag) = latest_github_release("liuchengxu", "vim-clap")
                .await
                .map(|r| r.tag_name)
            {
                download_prebuilt_binary(&latest_tag, true)
                    .await
                    .unwrap_or_else(|err| panic!(
                        "Failed to download the prebuilt binary for {latest_tag:?} into a tempfile: {err:?}"
                    ));
                return;
            }
            tokio::time::sleep(std::time::Duration::from_millis(300)).await;
        }

        panic!("Failed to download the prebuilt binary of latest release");
    }
}
