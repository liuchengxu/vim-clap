use dirs::BASE_DIRS;
use itertools::Itertools;
use once_cell::sync::Lazy;
use serde::de::Error as DeserializeError;
use serde::{Deserialize, Deserializer, Serialize};
use std::borrow::Cow;
use std::convert::TryFrom;
use std::fs::canonicalize;
use std::path::{Display, Path, PathBuf, MAIN_SEPARATOR};

/// Unit type wrapper of [`PathBuf`] that is absolute path.
#[derive(Debug, Clone, Ord, PartialOrd, Eq, PartialEq, Hash, Serialize)]
pub struct AbsPathBuf(PathBuf);

impl<'de> Deserialize<'de> for AbsPathBuf {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let path = PathBuf::deserialize(deserializer)?;
        if path.is_absolute() {
            Ok(Self(path))
        } else if let Ok(stripped) = path.strip_prefix("~") {
            let path = BASE_DIRS.home_dir().join(stripped);
            // Resolve the symlink.
            let path =
                canonicalize(path).map_err(|err| DeserializeError::custom(err.to_string()))?;
            Ok(Self(path))
        } else {
            let path = canonicalize(&path).map_err(|err| {
                DeserializeError::custom(format!("Can not canonicalize {}: {err}", path.display()))
            })?;
            if path.is_absolute() {
                Ok(Self(path))
            } else {
                Err(DeserializeError::custom(
                    "Can not convert {path} to absolute form, \
                    please specify it as absolute path directly",
                ))
            }
        }
    }
}

impl AbsPathBuf {
    pub fn display(&self) -> Display<'_> {
        self.0.display()
    }

    /// # Panics
    ///
    /// Panics if path contains invalid unicode.
    pub fn as_str(&self) -> &str {
        self.0
            .to_str()
            .unwrap_or_else(|| panic!("{} contains invalid unicode", self.0.display()))
    }
}

impl From<AbsPathBuf> for PathBuf {
    fn from(abs_path_buf: AbsPathBuf) -> PathBuf {
        abs_path_buf.0
    }
}

impl TryFrom<PathBuf> for AbsPathBuf {
    type Error = PathBuf;
    fn try_from(path_buf: PathBuf) -> Result<AbsPathBuf, PathBuf> {
        if !path_buf.is_absolute() {
            return Err(path_buf);
        }
        Ok(Self(path_buf))
    }
}

impl TryFrom<&str> for AbsPathBuf {
    type Error = PathBuf;
    fn try_from(path: &str) -> Result<AbsPathBuf, PathBuf> {
        Self::try_from(PathBuf::from(path))
    }
}

impl std::ops::Deref for AbsPathBuf {
    type Target = PathBuf;
    fn deref(&self) -> &PathBuf {
        &self.0
    }
}

impl AsRef<Path> for AbsPathBuf {
    fn as_ref(&self) -> &Path {
        self.0.as_path()
    }
}

impl std::str::FromStr for AbsPathBuf {
    type Err = &'static str;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self::try_from(s)
            .unwrap_or_else(|path| panic!("expected absolute path, got {}", path.display())))
    }
}

impl std::fmt::Display for AbsPathBuf {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0.display())
    }
}

pub fn expand_tilde(path: impl AsRef<str>) -> PathBuf {
    static HOME_PREFIX: Lazy<String> = Lazy::new(|| format!("~{MAIN_SEPARATOR}"));

    if let Some(stripped) = path.as_ref().strip_prefix(HOME_PREFIX.as_str()) {
        BASE_DIRS.home_dir().join(stripped)
    } else {
        path.as_ref().into()
    }
}

// /home/xlc/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/lib/rustlib/src/rust/library/alloc/src/string.rs
pub fn truncate_absolute_path(abs_path: &str, max_len: usize) -> Cow<'_, str> {
    if abs_path.len() > max_len {
        let gap = abs_path.len() - max_len;

        if let Some(home_dir) = BASE_DIRS.home_dir().to_str() {
            if abs_path.starts_with(home_dir) {
                // ~/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/lib/rustlib/src/rust/library/alloc/src/string.rs
                if home_dir.len() > gap {
                    return abs_path.replacen(home_dir, "~", 1).into();
                }

                // ~/.rustup/.../github.com/paritytech/substrate/frame/system/src/lib.rs
                let relative_home_path = &abs_path.trim_start_matches(home_dir)[1..];
                if let Some((head, tail)) = relative_home_path.split_once(MAIN_SEPARATOR) {
                    let mut to_hide = 0usize;
                    for component in tail.split(MAIN_SEPARATOR) {
                        if to_hide > gap + 2 {
                            let mut tail = tail.to_string();
                            tail.replace_range(..to_hide - 1, "...");
                            return format!("~{MAIN_SEPARATOR}{head}{MAIN_SEPARATOR}{tail}").into();
                        } else {
                            to_hide += component.len() + 1;
                        }
                    }
                }
            } else {
                let top = abs_path.splitn(8, MAIN_SEPARATOR).collect::<Vec<_>>();
                if let Some(last) = top.last() {
                    if let Some((_head, tail)) = last.split_once(MAIN_SEPARATOR) {
                        let mut to_hide = 0usize;
                        for component in tail.split(MAIN_SEPARATOR) {
                            if to_hide > gap + 2 {
                                let mut tail = tail.to_string();
                                tail.replace_range(..to_hide - 1, "...");
                                let head = top
                                    .iter()
                                    .take(top.len() - 1)
                                    .join(MAIN_SEPARATOR.to_string().as_str());
                                return format!("{head}{MAIN_SEPARATOR}{tail}").into();
                            } else {
                                to_hide += component.len() + 1;
                            }
                        }
                    }
                }
            }
        } else {
            // Truncate the left of absolute path string.
            // ../stable-x86_64-unknown-linux-gnu/lib/rustlib/src/rust/library/alloc/src/string.rs
            if let Some((offset, _)) = abs_path.char_indices().nth(abs_path.len() - max_len + 2) {
                let mut abs_path = abs_path.to_string();
                abs_path.replace_range(..offset, "..");
                return abs_path.into();
            }
        }
    }

    abs_path.into()
}

pub fn find_project_root<'a, P: AsRef<Path>>(
    start_dir: &'a Path,
    root_markers: &[P],
) -> Option<&'a Path> {
    upward_search(start_dir, |path| {
        root_markers
            .iter()
            .any(|root_marker| path.join(root_marker).exists())
    })
    .ok()
}

pub fn find_git_root(start_dir: &Path) -> Option<&Path> {
    upward_search(start_dir, |path| {
        [".git", ".git/"]
            .iter()
            .any(|root_marker| path.join(root_marker).exists())
    })
    .ok()
}

fn upward_search<F>(path: &Path, predicate: F) -> std::io::Result<&Path>
where
    F: Fn(&Path) -> bool,
{
    if predicate(path) {
        return Ok(path);
    }

    let next_path = path
        .parent()
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "No parent directory"))?;

    upward_search(next_path, predicate)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "Not sure why the behavior is differnt in CI"]
    fn test_truncate_absolute_path() {
        #[cfg(not(target_os = "windows"))]
        let p = ".rustup/toolchains/stable-x86_64-unknown-linux-gnu/lib/rustlib/src/rust/library/alloc/src/string.rs";
        #[cfg(target_os = "windows")]
        let p = r#".rustup\toolchains\stable-x86_64-unknown-linux-gnu\lib\rustlib\src\rust\library\alloc\src\string.rs"#;
        let abs_path = format!(
            "{}{MAIN_SEPARATOR}{p}",
            BASE_DIRS.home_dir().to_str().unwrap(),
        );
        let max_len = 60;
        #[cfg(not(target_os = "windows"))]
        let expected = "~/.rustup/.../src/rust/library/alloc/src/string.rs";
        #[cfg(target_os = "windows")]
        let expected = r#"~\.rustup\...\src\rust\library\alloc\src\string.rs"#;
        assert_eq!(truncate_absolute_path(&abs_path, max_len), expected);

        let abs_path = "/media/xlc/Data/src/github.com/paritytech/substrate/bin/node/cli/src/command_helper.rs";
        let expected = "/media/xlc/.../bin/node/cli/src/command_helper.rs";
        assert_eq!(truncate_absolute_path(abs_path, max_len), expected);

        let abs_path =
            "/Users/xuliucheng/src/github.com/subspace/subspace/crates/pallet-domains/src/lib.rs";
        println!("{:?}", truncate_absolute_path(abs_path, 60));
    }
}
