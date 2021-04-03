use std::fs::File;
use std::io::copy;
use std::path::{Path, PathBuf};

use anyhow::Result;

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
    let target = crate::github::download_url_for(version)?;

    let mut response = reqwest::blocking::get(&target)?;

    let (mut dest, temp_file) = {
        let fname = response
            .url()
            .path_segments()
            .and_then(|segments| segments.last())
            .and_then(|name| if name.is_empty() { None } else { Some(name) })
            .unwrap_or("tmp.bin");

        let mut tmp_dir = std::env::temp_dir();
        tmp_dir.push(format!("{}-{}", version, fname));
        (File::create(&tmp_dir)?, tmp_dir)
    };

    copy(&mut response, &mut dest)?;

    #[cfg(unix)]
    set_executable_permission(&temp_file)?;

    Ok(temp_file)
}

pub(super) async fn download_prebuilt_binary_to_a_tempfile_async(version: &str) -> Result<PathBuf> {
    use crate::github::{get_asset_name, retrieve_asset_size};
    use indicatif::{ProgressBar, ProgressStyle};
    use tokio::{fs, io::AsyncWriteExt};

    let asset_name = get_asset_name()?;
    let total_size = retrieve_asset_size(&asset_name, version)?;

    let client = reqwest::Client::new();
    let target = crate::github::download_url_for(version)?;
    let request = client.get(&target);

    let progress_bar = ProgressBar::new(total_size);
    progress_bar.set_style(ProgressStyle::default_bar()
                       .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})")
                       .progress_chars("#>-"));

    let mut tmp_dir = std::env::temp_dir();
    tmp_dir.push(format!("{}-{}", version, asset_name));

    if tmp_dir.is_file() {
        let metadata = std::fs::metadata(&tmp_dir)?;
        if metadata.len() == total_size {
            println!("{} has alreay been downloaded", tmp_dir.display());
            return Ok(tmp_dir);
        } else {
            std::fs::remove_file(&tmp_dir)?;
        }
    }

    let mut source = request.send().await?;

    let file = tmp_dir.as_path();
    let mut dest = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&file)
        .await?;

    while let Some(chunk) = source.chunk().await? {
        dest.write_all(&chunk).await?;
        progress_bar.inc(chunk.len() as u64);
    }

    #[cfg(unix)]
    set_executable_permission(&tmp_dir)?;

    println!("Download of '{}' has been completed.", tmp_dir.display());

    Ok(tmp_dir)
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
