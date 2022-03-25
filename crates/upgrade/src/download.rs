use std::fs::File;
use std::io::copy;
use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::github::download_url;

#[cfg(unix)]
fn set_executable_permission<P: AsRef<Path>>(path: P) -> Result<()> {
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
pub fn download_prebuilt_binary_to_a_tempfile(version: &str) -> Result<PathBuf> {
    let mut response = reqwest::blocking::get(&download_url(version)?)?;

    let (mut dest, temp_file) = {
        let fname = response
            .url()
            .path_segments()
            .and_then(|segments| segments.last())
            .and_then(|name| if name.is_empty() { None } else { Some(name) })
            .unwrap_or("tmp.bin");

        let mut tmp = std::env::temp_dir();
        tmp.push(format!("{}-{}", version, fname));
        (File::create(&tmp)?, tmp)
    };

    copy(&mut response, &mut dest)?;

    #[cfg(unix)]
    set_executable_permission(&temp_file)?;

    Ok(temp_file)
}

pub(super) async fn download_prebuilt_binary_to_a_tempfile_async(version: &str) -> Result<PathBuf> {
    use crate::github::{asset_name, retrieve_asset_size};
    use indicatif::{ProgressBar, ProgressStyle};
    use tokio::{fs, io::AsyncWriteExt};

    let asset_name = asset_name()?;
    let total_size = retrieve_asset_size(&asset_name, version)?;

    let client = reqwest::Client::new();
    let request = client.get(&download_url(version)?);

    let progress_bar = ProgressBar::new(total_size);
    progress_bar.set_style(ProgressStyle::default_bar()
                       .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})")
                       .progress_chars("#>-"));

    let mut tmp = std::env::temp_dir();
    tmp.push(format!("{}-{}", version, asset_name));

    if tmp.is_file() {
        let metadata = std::fs::metadata(&tmp)?;
        if metadata.len() == total_size {
            println!("{} has alreay been downloaded", tmp.display());
            return Ok(tmp);
        } else {
            std::fs::remove_file(&tmp)?;
        }
    }

    let mut source = request.send().await?;
    let mut dest = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&tmp.as_path())
        .await?;

    while let Some(chunk) = source.chunk().await? {
        dest.write_all(&chunk).await?;
        progress_bar.inc(chunk.len() as u64);
    }

    #[cfg(unix)]
    set_executable_permission(&tmp)?;

    println!("Download of '{}' has been completed.", tmp.display());

    Ok(tmp)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore]
    fn test_download_to_a_tempfile() {
        let remote_release = crate::github::latest_remote_release().unwrap();
        let remote_tag = remote_release.tag_name;
        download_prebuilt_binary_to_a_tempfile(&remote_tag).unwrap();
    }

    #[tokio::test]
    #[ignore]
    async fn test_async_download_to_a_tempfile() {
        let file = download_prebuilt_binary_to_a_tempfile_async("v0.14").await;
        println!("async downloaded file: {:?}", file);
    }
}
