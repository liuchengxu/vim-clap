// Now we do not need to genetate the constants module using the Python script.
// mod constants;
// pub use constants::*;
include!(concat!(env!("OUT_DIR"), "/constants.rs"));

use std::path::Path;

/// The type used to represent icons.
///
/// This could be changed into different type later,
/// so functions take and return this type, not `char` or `&str` directly.
pub type IconType = char;

pub const DEFAULT_ICON: IconType = '';
pub const FOLDER_ICON: IconType = '';
pub const DEFAULT_FILER_ICON: IconType = '';

/// Patched icon length in chars.
///
/// One char icon plus one space.
///
/// Matcher returns the indices in chars, but both Vim and Neovim add highlights
/// using the byte index, hence printer converts the char indices to the byte indices
/// before sending the final result to Vim/Neovim.
pub const ICON_CHAR_LEN: usize = 2;

#[derive(Debug, Clone, Copy, Default)]
pub enum Icon {
    #[default]
    Null,
    Enabled(IconKind),
    /// This variant is a mere flag indicating the icon is enabled but actually does not
    /// do anything on rendering the icon, which will be handled by ClapItem provider internally.
    ClapItem,
}

impl Icon {
    pub fn icon_kind(&self) -> Option<IconKind> {
        match self {
            Self::Enabled(icon_kind) => Some(*icon_kind),
            _ => None,
        }
    }

    pub fn enabled(&self) -> bool {
        matches!(self, Self::Enabled(_) | Self::ClapItem)
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
            "tags" | "buffer_tags" => Self::Enabled(IconKind::BufferTags),
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
    BufferTags,
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
    pub fn add_icon_to_text<S: AsRef<str>>(&self, text: S) -> String {
        let text = text.as_ref();
        let icon = self.icon(text);
        format!("{icon} {text}")
    }

    /// Returns appropriate icon for the given text.
    pub fn icon(&self, text: &str) -> IconType {
        match *self {
            Self::File => file_icon(text),
            Self::Grep => grep_icon(text),
            Self::ProjTags => proj_tags_icon(text),
            Self::BufferTags => buffer_tags_icon(text),
            Self::Unknown => DEFAULT_ICON,
        }
    }
}

/// Return appropriate icon for the path. If no icon matched, return the specified default one.
///
/// First try matching the [`EXACTMATCH_ICON_TABLE`] using the file name, and then finding the
/// [`EXTENSION_ICON_TABLE`] using the file extension.
fn get_icon_or<P: AsRef<Path>>(path: P, default: IconType) -> IconType {
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

pub fn icon_or_default(path: &Path) -> IconType {
    get_icon_or(path, DEFAULT_ICON)
}

fn buffer_tags_icon(line: &str) -> IconType {
    pattern::extract_buffer_tags_kind(line)
        .map(tags_kind_icon)
        .unwrap_or(DEFAULT_ICON)
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
        .map(file_icon)
        .unwrap_or(DEFAULT_ICON)
}

pub fn file_icon(line: &str) -> IconType {
    get_icon_or(Path::new(line), DEFAULT_ICON)
}

pub fn tags_kind_icon(kind: &str) -> IconType {
    bsearch_icon_table(kind, TAGKIND_ICON_TABLE)
        .map(|idx| TAGKIND_ICON_TABLE[idx].1)
        .unwrap_or(DEFAULT_ICON)
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
                let icon = format!("{i} ");
                // 4 bytes, 2 chars.
                assert_eq!(icon.len(), 4);
                assert_eq!(icon.chars().count(), 2);
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
