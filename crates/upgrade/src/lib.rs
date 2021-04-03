//! This crate provides the features to upgrade maple executable.

mod download;
mod github;

use anyhow::{anyhow, Context, Result};
use structopt::StructOpt;

/// This command is only invoked when user uses the prebuilt binary, more specifically, exe in
/// vim-clap/bin/maple.
#[derive(StructOpt, Debug, Clone)]
pub struct Upgrade {
    /// Download if the local version mismatches the latest remote version.
    #[structopt(long)]
    pub download: bool,
    /// Disable the downloading progress_bar
    #[structopt(long)]
    pub no_progress_bar: bool,
}

impl Upgrade {
    pub async fn run(&self, local_tag: &str) -> Result<()> {
        println!("Retrieving the latest remote release info...");
        let remote_release = github::latest_remote_release()?;
        let remote_tag = remote_release.tag_name;
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
                    github::download_url_for(&remote_tag)?
                );
            }
        } else {
            println!("No newer release, current maple version: {}", remote_tag);
        }
        Ok(())
    }

    async fn download_prebuilt_binary(&self, version: &str) -> Result<()> {
        let bin_path = get_binary_path()?;
        let temp_file = self.download_to_tempfile(version).await?;

        // Move the downloaded binary to bin/maple
        std::fs::rename(temp_file, bin_path)?;

        Ok(())
    }

    async fn download_to_tempfile(&self, version: &str) -> Result<std::path::PathBuf> {
        if self.no_progress_bar {
            download::download_prebuilt_binary_to_a_tempfile(version)
        } else {
            download::download_prebuilt_binary_to_a_tempfile_async(version).await
        }
    }
}

/// The prebuilt binary is put at bin/maple.
fn get_binary_path() -> Result<impl AsRef<std::path::Path>> {
    let exe_dir = std::env::current_exe()?;
    let bin_dir = exe_dir
        .parent()
        .context("Couldn't get the parent of current exe")?;
    if !bin_dir.ends_with("bin") {
        return Err(anyhow!(
            "Current exe has to be under vim-clap/bin directory"
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
    let v = remote_tag.split('.').collect::<Vec<_>>();
    v[1].parse()
        .unwrap_or_else(|_| panic!("Couldn't extract remote version"))
}

/// local: "v0.13-4-g58738c0"
#[inline]
fn extract_local_version_number(local_tag: &str) -> u32 {
    let tag = local_tag.split('-').collect::<Vec<_>>();
    extract_remote_version_number(tag[0])
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
