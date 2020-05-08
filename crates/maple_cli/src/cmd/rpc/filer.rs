use super::{write_response, Message};
use anyhow::Result;
use icon::prepend_filer_icon;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::path::PathBuf;
use std::{fs, io};

/// Display the inner path in a nicer way.
struct DisplayPath {
    inner: PathBuf,
    enable_icon: bool,
}

impl DisplayPath {
    pub fn new(path: PathBuf, enable_icon: bool) -> Self {
        Self {
            inner: path,
            enable_icon,
        }
    }

    #[inline]
    fn to_file_name_str(&self) -> Option<&str> {
        self.inner.file_name().and_then(std::ffi::OsStr::to_str)
    }
}

impl Into<String> for DisplayPath {
    fn into(self) -> String {
        let path_str = if self.inner.is_dir() {
            // TODO: windows compatible?
            format!("{}/", self.to_file_name_str().unwrap())
        } else {
            self.to_file_name_str().map(Into::into).unwrap()
        };

        if self.enable_icon {
            prepend_filer_icon(&self.inner, &path_str)
        } else {
            path_str
        }
    }
}

fn read_dir_entries(dir: &str, enable_icon: bool) -> Result<Vec<String>> {
    let mut entries = fs::read_dir(dir)?
        .map(|res| res.map(|x| DisplayPath::new(x.path(), enable_icon).into()))
        .collect::<Result<Vec<_>, io::Error>>()?;

    entries.sort();

    Ok(entries)
}

#[derive(Serialize, Deserialize)]
struct FilerParams {
    cwd: String,
    enable_icon: bool,
}

impl From<serde_json::Map<String, serde_json::Value>> for FilerParams {
    fn from(serde_map: serde_json::Map<String, serde_json::Value>) -> Self {
        Self {
            cwd: String::from(
                serde_map
                    .get("cwd")
                    .and_then(|x| x.as_str())
                    .unwrap_or("Missing cwd when deserializing into FilerParams"),
            ),
            enable_icon: serde_map
                .get("enable_icon")
                .and_then(|x| x.as_bool())
                .unwrap_or(false),
        }
    }
}

pub(super) fn handle_message(msg: Message) {
    let FilerParams { cwd, enable_icon } = msg.params.into();

    let result = match read_dir_entries(&cwd, enable_icon) {
        Ok(entries) => {
            let result = json!({
            "entries": entries,
            "dir": cwd,
            "total": entries.len(),
            });
            json!({ "result": result, "id": msg.id })
        }
        Err(err) => {
            let error = json!({"message": format!("{}", err), "dir": cwd});
            json!({ "error": error, "id": msg.id })
        }
    };

    write_response(result);
}

#[test]
fn test_dir() {
    let entries = read_dir_entries(
        &std::env::current_dir()
            .unwrap()
            .into_os_string()
            .into_string()
            .unwrap(),
        false,
    )
    .unwrap();
    println!("entry: {:?}", entries);
}
