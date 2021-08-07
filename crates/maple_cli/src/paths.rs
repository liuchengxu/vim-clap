use std::convert::TryFrom;
use std::path::{Display, Path, PathBuf};

/// Unit type wrapper of [`PathBuf`] that is absolute path.
#[derive(Debug, Clone, Ord, PartialOrd, Eq, PartialEq, Hash)]
pub struct AbsPathBuf(PathBuf);

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
