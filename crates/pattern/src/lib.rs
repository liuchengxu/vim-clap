//! Regex patterns and utilities used for manipulating the line.

use lazy_static::lazy_static;
use log::error;
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

  static ref COMMIT_RE: Regex = Regex::new(r"^.*\d{4}-\d{2}-\d{2}\s+([0-9a-z]+)\s+").unwrap();
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
    let str2nr = |idx: usize| cap.get(idx).map(|x| x.as_str()).and_then(parse_lnum);
    let lnum = str2nr(2)?;
    let col = str2nr(3)?;
    Some((fpath, lnum, col))
}

pub fn extract_grep_file_path(line: &str) -> Option<String> {
    let cap = GREP_POS.captures(line)?;
    cap.get(1).map(|x| x.as_str().into())
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

fn parse_lnum(lnum: &str) -> Option<usize> {
    match lnum.parse::<usize>() {
        Err(e) => {
            error!("failed to extract lnum from {}, error:{:?}", lnum, e);
            None
        }
        Ok(p) => Some(p),
    }
}

pub fn parse_rev(line: &str) -> Option<&str> {
    let cap = COMMIT_RE.captures(line)?;
    cap.get(1).map(|x| x.as_str())
}

pub fn extract_proj_tags(line: &str) -> Option<(usize, &str)> {
    let cap = PROJ_TAGS.captures(line)?;
    let lnum = cap.get(2).map(|x| x.as_str()).and_then(parse_lnum)?;
    let fpath = cap.get(4).map(|x| x.as_str())?;
    Some((lnum, fpath))
}

pub fn extract_proj_tags_kind(line: &str) -> Option<&str> {
    let cap = PROJ_TAGS.captures(line)?;
    let kind = cap.get(3).map(|x| x.as_str())?;
    Some(kind)
}

pub fn extract_buf_tags_lnum(line: &str) -> Option<usize> {
    let cap = BUFFER_TAGS.captures(line)?;
    cap.get(1).map(|x| x.as_str()).and_then(parse_lnum)
}

pub fn extract_blines_lnum(line: &str) -> Option<usize> {
    line.split_whitespace().next().and_then(parse_lnum)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_grep_regex() {
        let line = "install.sh:1:5:#!/usr/bin/env bash";
        let e = extract_grep_position(line).unwrap();
        assert_eq!(("install.sh".into(), 1, 5), e);

        let path = extract_grep_file_path(line).unwrap();
        assert_eq!(path, "install.sh");
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
        assert_eq!(Some(58), extract_buf_tags_lnum(line));
    }

    #[test]
    fn test_blines_lnum() {
        let line = r#" 103       call clap#helper#echo_error('Provider without source must specify on_moved, but only has: '.keys(provider_info))"#;
        assert_eq!(Some(103), extract_blines_lnum(line));
    }

    #[test]
    fn test_parse_rev() {
        let line =
            "* 2019-10-18 8ed4391 Rename sign and rooter related options (#65) (Liu-Cheng Xu)";
        assert_eq!(parse_rev(line), Some("8ed4391"));
        let line = "2019-10-18 8ed4391 Rename sign and rooter related options (#65) (Liu-Cheng Xu)";
        assert_eq!(parse_rev(line), Some("8ed4391"));
        let line = "2019-12-29 3f0d00c Add forerunner job status sign and a delay timer for running maple (#184) (Liu-Cheng Xu)";
        assert_eq!(parse_rev(line), Some("3f0d00c"));
    }
}
