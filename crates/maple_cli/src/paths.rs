use serde::de::Error as DeserializeError;
use serde::{Deserialize, Deserializer, Serialize};
use std::convert::TryFrom;
use std::fs::canonicalize;
use std::path::{Display, Path, PathBuf};

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
            let path = crate::utils::HOME_DIR.clone().join(stripped);
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
                Err(DeserializeError::custom("Can not convert {path} to absolute form, please specify it as absolute path directly"))
            }
        }
    }
}

impl AbsPathBuf {
    pub fn display(&self) -> Display<'_> {
        self.0.display()
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
