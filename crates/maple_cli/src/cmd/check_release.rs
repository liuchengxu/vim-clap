use anyhow::Result;
use curl::easy::{Easy, List};
use serde::{Deserialize, Serialize};

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

#[test]
fn test_curl() {
    println!("{:?}", check_new_release().unwrap());
}
