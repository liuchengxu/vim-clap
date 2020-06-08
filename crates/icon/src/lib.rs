mod constants;

pub use constants::*;

use std::path::Path;
use structopt::clap::arg_enum;

pub const DEFAULT_ICON: char = '';
pub const FOLDER_ICON: char = '';
pub const DEFAULT_FILER_ICON: char = '';

// Each added icon length is 4 bytes.
pub const ICON_LEN: usize = 4;

/// The type used to represent icons.
///
/// This could be changed into different type later,
/// so functions take and return this type, not `char` or `&str` directly.
type Icon = char;

/// Return appropriate icon for the path. If no icon matched, return the specified default one.
///
/// Try matching the exactmatch map against the file name, and then the extension map.
#[inline]
pub fn get_icon_or(path: &Path, default: Icon) -> Icon {
    path.file_name()
        .and_then(std::ffi::OsStr::to_str)
        .and_then(|filename| {
            bsearch_icon_table(&filename.to_lowercase().as_str(), EXACTMATCH_ICON_TABLE)
                .map(|idx| EXACTMATCH_ICON_TABLE[idx].1)
        })
        .unwrap_or_else(|| {
            path.extension()
                .and_then(std::ffi::OsStr::to_str)
                .and_then(|ext| {
                    bsearch_icon_table(ext, EXTENSION_ICON_TABLE)
                        .map(|idx| EXTENSION_ICON_TABLE[idx].1)
                })
                .unwrap_or(default)
        })
}

fn icon_for(line: &str) -> Icon {
    let path = Path::new(line);
    get_icon_or(&path, DEFAULT_ICON)
}

pub fn prepend_icon(line: &str) -> String {
    format!("{} {}", icon_for(line), line)
}

#[inline]
pub fn icon_for_filer(path: &Path) -> Icon {
    if path.is_dir() {
        FOLDER_ICON
    } else {
        get_icon_or(path, DEFAULT_FILER_ICON)
    }
}

pub fn prepend_filer_icon(path: &Path, line: &str) -> String {
    format!("{} {}", icon_for_filer(path), line)
}

fn get_tagkind_icon(line: &str) -> Icon {
    pattern::extract_proj_tags_kind(line)
        .and_then(|kind| {
            bsearch_icon_table(kind, TAGKIND_ICON_TABLE).map(|idx| TAGKIND_ICON_TABLE[idx].1)
        })
        .unwrap_or(DEFAULT_ICON)
}

#[inline]
fn grep_icon_for(line: &str) -> Icon {
    pattern::extract_fpath_from_grep_line(line)
        .map(|fpath| icon_for(fpath))
        .unwrap_or(DEFAULT_ICON)
}

/// Prepend an icon to the output line of ripgrep.
pub fn prepend_grep_icon(line: &str) -> String {
    format!("{} {}", grep_icon_for(line), line)
}

arg_enum! {
  /// Prepend an icon for various kind of output line.
  #[derive(Clone, Debug)]
  pub enum IconPainter {
      File,
      Grep,
      ProjTags
  }
}

impl IconPainter {
    /// Returns a `String` of raw str with icon added.
    pub fn paint(&self, raw_str: &str) -> String {
        match *self {
            Self::File => prepend_icon(raw_str),
            Self::Grep => prepend_grep_icon(raw_str),
            Self::ProjTags => format!("{} {}", get_tagkind_icon(raw_str), raw_str),
        }
    }

    /// Returns appropriate icon for the given text.
    pub fn get_icon(&self, text: &str) -> Icon {
        match *self {
            Self::File => icon_for(text),
            Self::Grep => grep_icon_for(text),
            Self::ProjTags => get_tagkind_icon(text),
        }
    }
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
        let icon_for = |kind: &str| {
            bsearch_icon_table(kind, TAGKIND_ICON_TABLE).map(|idx| TAGKIND_ICON_TABLE[idx].1)
        };
        assert_eq!(icon_for("implementation").unwrap(), get_tagkind_icon(line));
    }
}
