//! Regex patterns and utilities used for manipulating the line.

use lazy_static::lazy_static;
use regex::Regex;
use std::path::PathBuf;

lazy_static! {
  static ref GREP_POS: Regex = Regex::new(r"^(.*):(\d+):(\d+):").unwrap();

  // match the file path and line number of grep line.
  static ref GREP_STRIP_FPATH: Regex = Regex::new(r"^.*:\d+:\d+:").unwrap();

  // match the tag_name:lnum of tag line.
  static ref TAG_RE: Regex = Regex::new(r"^(.*:\d+)").unwrap();

  static ref BUFFER_TAGS: Regex = Regex::new(r"^.*:(\d+)").unwrap();

  static ref PROJ_TAGS: Regex = Regex::new(r"^(.*):(\d+).*\[(.*)@(.*)\]").unwrap();
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
/// //                                <----       line content       ---->
/// // crates/printer/src/lib.rs:199:26:        let query = "srlisrlisrsr";
/// //                                |
/// //                             offset
#[inline]
pub fn strip_grep_filepath(line: &str) -> Option<(&str, usize)> {
    GREP_STRIP_FPATH
        .find(line)
        .map(|mat| (&line[mat.end()..], mat.end()))
}

/// Returns a tuple of (fpath, lnum, col).
pub fn extract_grep_position(line: &str) -> Option<(PathBuf, usize, usize)> {
    let cap = GREP_POS.captures(line)?;
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

/// Returns fpath part in grep line.
#[inline]
pub fn extract_fpath_from_grep_line(line: &str) -> Option<&str> {
    GREP_POS
        .captures(line)
        .and_then(|cap| cap.get(1).map(|x| x.as_str()))
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

pub fn extract_proj_tags(line: &str) -> Option<(usize, &str)> {
    let cap = PROJ_TAGS.captures(line)?;
    let lnum = cap
        .get(2)
        .map(|x| x.as_str())
        .map(|x| x.parse::<usize>().expect("\\d+ matched"))?;
    let fpath = cap.get(4).map(|x| x.as_str())?;
    Some((lnum, fpath))
}

pub fn extract_buf_tags_lnum(line: &str) -> Option<usize> {
    let cap = BUFFER_TAGS.captures(line)?;
    cap.get(1)
        .map(|x| x.as_str())
        .map(|x| x.parse::<usize>().expect("\\d+ matched"))
}

pub fn extract_blines_lnum(line: &str) -> Option<usize> {
    line.split_whitespace()
        .next()
        .map(|x| x.parse::<usize>().expect("\\d+ matched"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_grep_regex() {
        let line = "install.sh:1:5:#!/usr/bin/env bash";
        let e = extract_grep_position(line).unwrap();
        assert_eq!(("install.sh".into(), 1, 5), e);
    }

    #[test]
    fn test_tag_name_only() {
        let line = "<Backspace>:60       [map]           inoremap <silent> <buffer> <Backspace> <C-R>=clap#handler#bs_action()<CR>  ftplugin/clap_input.vim";
        let mat = TAG_RE.find(line);
        assert_eq!(mat.unwrap().as_str(), "<Backspace>:60");
    }

    #[test]
    fn test_proj_tags_regexp() {
        let line = r#"<C-D>:42                       [map@ftplugin/clap_input.vim]  inoremap <silent> <buffer> <expr> <C-D> col('.')>strlen(getline('.'))?"\\<Lt>C-D>":"\\<Lt>Del"#;
        assert_eq!(
            (42, "ftplugin/clap_input.vim"),
            extract_proj_tags(line).unwrap()
        );
    }

    #[test]
    fn test_buffer_tags_regexp() {
        let line = r#" extract_fpath_from_grep_line:58  [function]  pub fn extract_fpath_from_grep_line(line: &str) -> Option<&str> {"#;
        println!("{:?}", extract_buf_tags_lnum(line));
    }

    #[test]
    fn test_blines_lnum() {
        let line = r#" 103       call clap#helper#echo_error('Provider without source must specify on_moved, but only has: '.keys(provider_info))"#;
        println!("{:?}", extract_blines_lnum(line));
    }
}
