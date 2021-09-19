// Now we do not need to genetate the constants module using the Python script.
// mod constants;
// pub use constants::*;
include!(concat!(env!("OUT_DIR"), "/constants.rs"));

use std::path::Path;

pub const DEFAULT_ICON: char = '';
pub const FOLDER_ICON: char = '';
pub const DEFAULT_FILER_ICON: char = '';

// Each added icon length is 4 bytes.
pub const ICON_LEN: usize = 4;

#[derive(Debug, Clone, Copy)]
pub enum Icon {
    Null,
    Enabled(IconKind),
}

impl Default for Icon {
    fn default() -> Self {
        Self::Null
    }
}

impl Icon {
    pub fn painter(&self) -> Option<&IconKind> {
        match self {
            Self::Null => None,
            Self::Enabled(icon_kind) => Some(icon_kind),
        }
    }
}

impl std::str::FromStr for Icon {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(s.into())
    }
}

impl<T: AsRef<str>> From<T> for Icon {
    fn from(icon: T) -> Self {
        match icon.as_ref().to_lowercase().as_str() {
            "file" => Self::Enabled(IconKind::File),
            "grep" => Self::Enabled(IconKind::Grep),
            "projtags" | "proj_tags" => Self::Enabled(IconKind::ProjTags),
            _ => Self::Null,
        }
    }
}

/// This type represents the kind of various provider line format.
#[derive(Clone, Debug, Copy)]
pub enum IconKind {
    File,
    Grep,
    ProjTags,
    Unknown,
}

impl std::str::FromStr for IconKind {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(s.into())
    }
}

impl<T: AsRef<str>> From<T> for IconKind {
    fn from(icon: T) -> Self {
        match icon.as_ref().to_lowercase().as_str() {
            "file" => Self::File,
            "grep" => Self::Grep,
            "projtags" | "proj_tags" => Self::ProjTags,
            _ => Self::Unknown,
        }
    }
}

impl IconKind {
    /// Returns a `String` of raw str with icon added.
    pub fn paint<S: AsRef<str>>(&self, raw_str: S) -> String {
        let fmt = |s| format!("{} {}", s, raw_str.as_ref());
        match *self {
            Self::File => prepend_icon(raw_str.as_ref()),
            Self::Grep => prepend_grep_icon(raw_str.as_ref()),
            Self::ProjTags => fmt(proj_tags_icon(raw_str.as_ref())),
            Self::Unknown => fmt(DEFAULT_ICON),
        }
    }

    /// Returns appropriate icon for the given text.
    pub fn icon(&self, text: &str) -> IconType {
        match *self {
            Self::File => file_icon(text),
            Self::Grep => grep_icon(text),
            Self::ProjTags => proj_tags_icon(text),
            Self::Unknown => DEFAULT_ICON,
        }
    }
}

/// The type used to represent icons.
///
/// This could be changed into different type later,
/// so functions take and return this type, not `char` or `&str` directly.
type IconType = char;

/// Return appropriate icon for the path. If no icon matched, return the specified default one.
///
/// Try matching the exactmatch map against the file name, and then the extension map.
#[inline]
pub fn get_icon_or<P: AsRef<Path>>(path: P, default: IconType) -> IconType {
    path.as_ref()
        .file_name()
        .and_then(std::ffi::OsStr::to_str)
        .and_then(|filename| {
            bsearch_icon_table(filename.to_lowercase().as_str(), EXACTMATCH_ICON_TABLE)
                .map(|idx| EXACTMATCH_ICON_TABLE[idx].1)
        })
        .unwrap_or_else(|| {
            path.as_ref()
                .extension()
                .and_then(std::ffi::OsStr::to_str)
                .and_then(|ext| {
                    bsearch_icon_table(ext, EXTENSION_ICON_TABLE)
                        .map(|idx| EXTENSION_ICON_TABLE[idx].1)
                })
                .unwrap_or(default)
        })
}

pub fn file_icon(line: &str) -> IconType {
    let path = Path::new(line);
    get_icon_or(&path, DEFAULT_ICON)
}

pub fn prepend_icon(line: &str) -> String {
    format!("{} {}", file_icon(line), line)
}

#[inline]
pub fn filer_icon<P: AsRef<Path>>(path: P) -> IconType {
    if path.as_ref().is_dir() {
        FOLDER_ICON
    } else {
        get_icon_or(path, DEFAULT_FILER_ICON)
    }
}

pub fn prepend_filer_icon<P: AsRef<Path>>(path: P, line: &str) -> String {
    format!("{} {}", filer_icon(path), line)
}

fn proj_tags_icon(line: &str) -> IconType {
    pattern::extract_proj_tags_kind(line)
        .and_then(|kind| {
            bsearch_icon_table(kind, TAGKIND_ICON_TABLE).map(|idx| TAGKIND_ICON_TABLE[idx].1)
        })
        .unwrap_or(DEFAULT_ICON)
}

#[inline]
fn grep_icon(line: &str) -> IconType {
    pattern::extract_fpath_from_grep_line(line)
        .map(|fpath| file_icon(fpath))
        .unwrap_or(DEFAULT_ICON)
}

/// Prepend an icon to the output line of ripgrep.
pub fn prepend_grep_icon(line: &str) -> String {
    format!("{} {}", grep_icon(line), line)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trim_trailing() {
        let empty_iconized_line = " ";
        assert_eq!(empty_iconized_line.len(), 4);
        assert!(empty_iconized_line.chars().next().unwrap() == DEFAULT_ICON);
    }

    #[test]
    fn test_icon_length() {
        for table in [EXTENSION_ICON_TABLE, EXACTMATCH_ICON_TABLE].iter() {
            for (_, i) in table.iter() {
                let icon = format!("{} ", i);
                assert_eq!(icon.len(), 4);
            }
        }
    }

    #[test]
    fn test_tagkind_icon() {
        let line = r#"Blines:19                      [implementation@crates/maple_cli/src/cmd/blines.rs] impl Blines {"#;
        let file_icon = |kind: &str| {
            bsearch_icon_table(kind, TAGKIND_ICON_TABLE).map(|idx| TAGKIND_ICON_TABLE[idx].1)
        };
        assert_eq!(file_icon("implementation").unwrap(), proj_tags_icon(line));
    }
}
