use super::{REPO, USER};
use anyhow::{anyhow, Result};
use std::fs::File;
use std::io::copy;
use std::path::{Path, PathBuf};

fn get_asset_name() -> Result<String> {
    let asset_name = if cfg!(target_os = "macos") {
        "maple-x86_64-apple-darwin"
    } else if cfg!(target_os = "linux") {
        "maple-x86_64-unknown-linux-gnu"
    } else if cfg!(target_os = "windows") {
        "maple-x86_64-pc-windows-msvc"
    } else {
        return Err(anyhow!("no-avaliable-prebuilt-binary for this platform"));
    };
    Ok(asset_name.into())
}

pub fn to_download_url(version: &str) -> Result<String> {
    Ok(format!(
        "https://github.com/{}/{}/releases/download/{}/{}",
        USER,
        REPO,
        version,
        get_asset_name()?
    ))
}

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
    let target = to_download_url(version)?;

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore]
    fn test_download_to_a_tempfile() {
        let remote_release = crate::cmd::check_release::latest_remote_release().unwrap();
        let remote_tag = remote_release.tag_name;
        download_prebuilt_binary_to_a_tempfile(&remote_tag).unwrap();
    }
}
