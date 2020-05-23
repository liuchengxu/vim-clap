use lazy_static::lazy_static;
use regex::Regex;
use std::path::PathBuf;

lazy_static! {
  static ref GREP_POS: Regex = Regex::new(r"^(.*):(\d+):(\d+):").unwrap();

  // match the file path and line number of grep line.
  static ref GREP_STRIP_FPATH: Regex = Regex::new(r"^.*:\d+:\d+:").unwrap();

  // match the tag_name:lnum of tag line.
  static ref TAG_RE: Regex = Regex::new(r"^(.*:\d+)").unwrap();
}

/// Extract tag name from the line in tags provider.
#[inline]
pub fn tag_name_only(line: &str) -> Option<&str> {
    TAG_RE.find(line).map(|x| x.as_str())
}

/// Returns the line content only and offset in the raw line.
///
/// Do not match the file path when using ripgrep.
///
///                                  |<----      line content     ---->|
/// crates/printer/src/lib.rs:199:26:        let query = "srlisrlisrsr";
///                                 |
///                              offset
#[inline]
pub fn strip_grep_filepath(line: &str) -> Option<(&str, usize)> {
    GREP_STRIP_FPATH
        .find(line)
        .map(|mat| (&line[mat.end()..], mat.end()))
}

/// Returns a tuple of (fpath, lnum, col).
pub fn extract_grep_position(line: &str) -> Option<(PathBuf, usize, usize)> {
    let cap = GREP_POS.captures(&line)?;
    let fpath = cap.get(1).map(|x| x.as_str().into())?;
    let str2nr = |idx: usize| {
        cap.get(idx)
            .map(|x| x.as_str())
            .map(|x| x.parse::<usize>().expect("\\d+ matched"))
    };
    let lnum = str2nr(2)?;
    let col = str2nr(3)?;
    Some((fpath, lnum, col))
}

/// Returns the file name of files entry.
#[inline]
pub fn file_name_only(line: &str) -> Option<(&str, usize)> {
    let fpath: std::path::PathBuf = line.into();
    fpath
        .file_name()
        .map(|x| x.to_string_lossy().into_owned())
        .map(|fname| (&line[line.len() - fname.len()..], line.len() - fname.len()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exclude_grep_filepath() {
        let query = "rules";
        let line = "crates/maple_cli/src/lib.rs:2:1:macro_rules! println_json {";
        let (_, origin_indices) = fuzzy_indices_fzy(line, query).unwrap();
        let (_, indices) = apply_fzy_on_grep_line(line, query).unwrap();
        assert_eq!(origin_indices, indices);
    }

    #[test]
    fn test_file_name_only() {
        let query = "lib";
        let line = "crates/extracted_fzy/src/lib.rs";
        let (_, origin_indices) = fuzzy_indices_fzy(line, query).unwrap();
        let (_, indices) = apply_fzy_on_file_line(line, query).unwrap();
        assert_eq!(origin_indices, indices);
    }

    #[test]
    fn test_tag_name_only() {
        let line = "<Backspace>:60       [map]           inoremap <silent> <buffer> <Backspace> <C-R>=clap#handler#bs_action()<CR>  ftplugin/clap_input.vim";
        let mat = TAG_RE.find(line);
        assert_eq!(mat.unwrap().as_str(), "<Backspace>:60");
    }
}
