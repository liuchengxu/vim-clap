use std::path::{Path, PathBuf};

use indicatif::{ProgressBar, ProgressStyle};
use tokio::io::AsyncWriteExt;

use crate::github::{download_url, retrieve_asset_size, PLATFORM};

#[cfg(unix)]
fn set_executable_permission<P: AsRef<Path>>(path: P) -> std::io::Result<()> {
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
pub(super) async fn download_prebuilt_binary(
    version: &str,
    no_progress_bar: bool,
) -> std::io::Result<PathBuf> {
    let binary_unavailable = || {
        std::io::Error::new(
            std::io::ErrorKind::Other,
            "no-avaliable-prebuilt-binary for this platform",
        )
    };

    let asset_name = PLATFORM.as_asset_name().ok_or_else(binary_unavailable)?;

    let mut tmp = std::env::temp_dir();
    tmp.push(format!("{}-{}", version, asset_name));

    let total_size = retrieve_asset_size(asset_name, version).await?;

    // Check if there is a partially download binary before.
    if tmp.is_file() {
        let metadata = std::fs::metadata(&tmp)?;
        if metadata.len() == total_size {
            println!("{} has alreay been downloaded", tmp.display());
            return Ok(tmp);
        } else {
            std::fs::remove_file(&tmp)?;
        }
    }

    let mut maybe_progress_bar = if no_progress_bar {
        None
    } else {
        let progress_bar = ProgressBar::new(total_size);
        progress_bar.set_style(ProgressStyle::default_bar()
                       .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})")
                       .progress_chars("#>-"));
        Some(progress_bar)
    };

    let download_url = download_url(version).ok_or_else(binary_unavailable)?;
    let request = reqwest::Client::new().get(download_url);

    let to_io_error = |e| {
        std::io::Error::new(
            std::io::ErrorKind::Other,
            format!("There was an error while sending request: {e}"),
        )
    };

    let mut source = request.send().await.map_err(to_io_error)?;
    let mut dest = tokio::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&tmp.as_path())
        .await?;

    while let Some(chunk) = source.chunk().await.map_err(to_io_error)? {
        dest.write_all(&chunk).await?;

        if let Some(ref mut progress_bar) = maybe_progress_bar {
            progress_bar.inc(chunk.len() as u64);
        }
    }

    #[cfg(unix)]
    set_executable_permission(&tmp)?;

    println!("Download of '{}' has been completed.", tmp.display());

    Ok(tmp)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_download_prebuilt_binary() {
        download_prebuilt_binary("v0.34", true)
            .await
            .expect("Failed to download the prebuilt binary into a tempfile");
    }
}
