//! This crate provides the features to upgrade maple executable.

mod download;
mod github;

use anyhow::Result;

/// This command is only invoked when user uses the prebuilt binary, more specifically, exe in
/// vim-clap/bin/maple.
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

    pub async fn run(&self, local_tag: &str) -> Result<()> {
        println!("Retrieving the latest remote release info...");
        let latest_release = github::retrieve_latest_release().await?;
        let remote_tag = latest_release.tag_name;
        let remote_version = extract_remote_version_number(&remote_tag);
        let local_version = extract_local_version_number(local_tag);
        if remote_version != local_version {
            if self.download {
                println!(
                    "New maple release {} is avaliable, downloading...",
                    remote_tag
                );
                self.download_prebuilt_binary(&remote_tag).await?;
                println!("Latest version {} download completed", remote_tag);
            } else {
                println!(
                    "New maple release {} is avaliable, please download it from {} or rerun with --download flag.",
                    remote_tag,
                    github::download_url(&remote_tag)?
                );
            }
        } else {
            println!("No newer release, current maple version: {}", remote_tag);
        }
        Ok(())
    }

    async fn download_prebuilt_binary(&self, version: &str) -> Result<()> {
        let temp_file = download::download_prebuilt_binary(version, self.no_progress_bar).await?;

        let bin_path = get_binary_path()?;

        // Move the downloaded binary to bin/maple
        std::fs::rename(temp_file, bin_path)?;

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
        return Err(Error::new(
            ErrorKind::Other,
            "Current executable must be put vim-clap/bin directory",
        ));
    }

    #[cfg(windows)]
    let bin_path = bin_dir.join("maple.exe");
    #[cfg(not(windows))]
    let bin_path = bin_dir.join("maple");

    Ok(bin_path)
}

/// remote: "v0.13"
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
    let tag = local_tag.split('-').next().expect("Invalid tag");
    extract_remote_version_number(tag)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_version_number() {
        let tag = "v0.13-4-g58738c0";
        assert_eq!(13u32, extract_local_version_number(tag));
        let tag = "v0.13";
        assert_eq!(13u32, extract_local_version_number(tag));
    }
}
