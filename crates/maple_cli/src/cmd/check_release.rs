use anyhow::{anyhow, Context, Result};
use curl::easy::{Easy, List};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use structopt::StructOpt;

/// This command is only invoked when user uses the prebuilt binary, more specifically, exe in
/// vim-clap/bin/maple.
#[derive(StructOpt, Debug, Clone)]
pub struct CheckRelease {
    /// Download if the local version mismatches the latest remote version.
    #[structopt(long)]
    pub download: bool,
}

impl CheckRelease {
    pub fn check_new_release(&self, local_tag: &str) -> Result<()> {
        let remote_release = latest_remote_release()?;
        let remote_tag = remote_release.tag_name;
        let remote_tag = "v0.14";
        let remote_version = extract_remote_version_number(&remote_tag);
        let local_version = extract_local_version_number(local_tag);
        if remote_version != local_version {
            if self.download {
                // self.download_prebuilt_binary(&remote_tag)?;
                self.download_prebuilt_binary("v0.13")?;
                println!("Latest version {} download completed", remote_tag);
            } else {
                println!(
                    "New maple release {} is avaliable, please download it from {} or rerun with --download flag.",
                    remote_tag,
                    download_url(&remote_tag)
                );
            }
        } else {
            println!("No newer release, current maple version: {}", remote_tag);
        }
        Ok(())
    }

    fn download_prebuilt_binary(&self, version: &str) -> Result<()> {
        let exe_dir = std::env::current_exe()?;
        let bin_dir = exe_dir
            .parent()
            .context("Couldn't get the parent of current exe")?;
        if !bin_dir.ends_with("bin") {
            return Err(anyhow!(
                "Current exe has to be under vim-clap/bin directory"
            ));
        }
        let temp_file = download_prebuilt_binary_to_a_tempfile(version)?;
        println!("bin_dir: {:?}", bin_dir);
        #[cfg(windows)]
        let bin_path = bin_dir.join("maple.exe");
        #[cfg(not(windows))]
        let bin_path = bin_dir.join("maple");
        // Move the downloaded binary to bin/maple
        std::fs::rename(temp_file, bin_path)?;
        Ok(())
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct RemoteRelease {
    pub tag_name: String,
}

fn get_raw_release_info() -> Result<Vec<u8>> {
    let mut dst = Vec::new();
    let mut handle = Easy::new();
    handle.url("https://api.github.com/repos/liuchengxu/vim-clap/releases/latest")?;
    let mut headers = List::new();
    headers.append("User-Agent: liuchengxu")?;
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

pub fn latest_remote_release() -> Result<RemoteRelease> {
    let data = get_raw_release_info()?;
    let release: RemoteRelease = serde_json::from_slice(&data).unwrap();
    Ok(release)
}

/// remote: "v0.13"
#[inline]
fn extract_remote_version_number(remote_tag: &str) -> u32 {
    let v = remote_tag.split('.').collect::<Vec<_>>();
    v[1].parse().expect("Couldn't extract remote version")
}

/// local: "v0.13-4-g58738c0"
#[inline]
fn extract_local_version_number(local_tag: &str) -> u32 {
    let tag = local_tag.split('-').collect::<Vec<_>>();
    extract_remote_version_number(tag[0])
}

#[cfg(unix)]
fn set_executable_permission<P: AsRef<Path>>(path: P) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mut perms = std::fs::metadata(path.as_ref())?.permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(path.as_ref(), perms)?;
    Ok(())
}

fn get_asset_name() -> String {
    let asset_name = if cfg!(target_os = "macos") {
        "maple-x86_64-apple-darwin"
    } else if cfg!(target_os = "linux") {
        "maple-x86_64-unknown-linux-gnu"
    } else if cfg!(target_os = "windows") {
        "maple-x86_64-pc-windows-msvc"
    } else {
        "no-avaliable-prebuilt-binary"
    };
    asset_name.into()
}

fn download_url(version: &str) -> String {
    format!(
        "https://github.com/liuchengxu/vim-clap/releases/download/{}/{}",
        version,
        get_asset_name()
    )
}

/// Downloads the latest remote release binary to a temp file.
///
/// # Arguments
///
/// - `version`: "v0.13"
fn download_prebuilt_binary_to_a_tempfile(version: &str) -> Result<PathBuf> {
    use std::fs::File;
    use std::io::copy;

    let target = download_url(version);

    let mut response = reqwest::blocking::get(&target)?;

    let mut tmp_dir = std::env::temp_dir();

    let (mut dest, temp_file) = {
        let fname = response
            .url()
            .path_segments()
            .and_then(|segments| segments.last())
            .and_then(|name| if name.is_empty() { None } else { Some(name) })
            .unwrap_or("tmp.bin");

        println!("file to download: '{}'", fname);
        tmp_dir.push(format!("{}-{}", version, fname));
        println!("will be located under: '{:?}'", tmp_dir);
        (File::create(&tmp_dir)?, tmp_dir)
    };

    copy(&mut response, &mut dest)?;

    #[cfg(unix)]
    set_executable_permission(&temp_file)?;

    Ok(temp_file)
}

#[test]
fn test_curl() {
    // println!("{:?}", check_new_release("v0.13-4-g58738c0").unwrap());
}

#[test]
fn test_extract_version_number() {
    let tag = "v0.13-4-g58738c0";
    assert_eq!(13u32, extract_local_version_number(tag));
    let tag = "v0.13";
    assert_eq!(13u32, extract_local_version_number(tag));
}

#[test]
fn test_download_to_tempfile() {
    download_to_a_tempfile().unwrap();
}
