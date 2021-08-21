//! Regex patterns and utilities used for manipulating the line.

use std::path::PathBuf;

use log::error;
use once_cell::sync::Lazy;
use regex::Regex;

static GREP_POS: Lazy<Regex> = Lazy::new(|| Regex::new(r"^(.*?):(\d+):(\d+):(.*)").unwrap());

static DUMB_JUMP_LINE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^\[(.*)\](.*?):(\d+):(\d+):").unwrap());

// match the file path and line number of grep line.
static GREP_STRIP_FPATH: Lazy<Regex> = Lazy::new(|| Regex::new(r"^.*:\d+:\d+:").unwrap());

// match the tag_name:lnum of tag line.
static TAG_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"^(.*:\d+)").unwrap());

static BUFFER_TAGS: Lazy<Regex> = Lazy::new(|| Regex::new(r"^.*:(\d+)").unwrap());

static PROJ_TAGS: Lazy<Regex> = Lazy::new(|| Regex::new(r"^(.*):(\d+).*\[(.*)@(.*?)\]").unwrap());

static COMMIT_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^.*\d{4}-\d{2}-\d{2}\s+([0-9a-z]+)\s+").unwrap());

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
pub fn extract_grep_position(line: &str) -> Option<(PathBuf, usize, usize, &str)> {
    let cap = GREP_POS.captures(line)?;
    let fpath = cap.get(1).map(|x| x.as_str().into())?;
    let str2nr = |idx: usize| cap.get(idx).map(|x| x.as_str()).and_then(parse_lnum);
    let lnum = str2nr(2)?;
    let col = str2nr(3)?;
    let line_content = cap.get(4).map(|x| x.as_str())?;
    Some((fpath, lnum, col, line_content))
}

/// Returns a tuple of (fpath, lnum, col).
pub fn extract_jump_line_info(line: &str) -> Option<(&str, PathBuf, usize, usize)> {
    let cap = DUMB_JUMP_LINE.captures(line)?;
    let def_kind = cap.get(1).map(|x| x.as_str())?;
    let fpath = cap.get(2).map(|x| x.as_str().into())?;
    let str2nr = |idx: usize| cap.get(idx).map(|x| x.as_str()).and_then(parse_lnum);
    let lnum = str2nr(3)?;
    let col = str2nr(4)?;
    Some((def_kind, fpath, lnum, col))
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
        assert_eq!(("install.sh".into(), 1, 5, "#!/usr/bin/env bash"), e);

        let path = extract_grep_file_path(line).unwrap();
        assert_eq!(path, "install.sh");

        let line = r#"/home/xlc/.vim/plugged/vim-clap/crates/pattern/src/lib.rs:36:1:/// // crates/printer/src/lib.rs:199:26:        let query = "srlisrlisrsr"#;
        assert_eq!(
            "/home/xlc/.vim/plugged/vim-clap/crates/pattern/src/lib.rs",
            extract_grep_file_path(line).unwrap()
        );
    }

    #[test]
    fn test_dumb_jump_line() {
        let line = "[variable]crates/maple_cli/src/stdio_server/session/context.rs:36:8:        let cwd = msg.get_cwd().into();";
        let info = extract_jump_line_info(line).unwrap();
        assert_eq!(
            info,
            (
                "variable".into(),
                "crates/maple_cli/src/stdio_server/session/context.rs".into(),
                36,
                8
            )
        );
        let line = "[variable]crates/maple_cli/src/stdio_server/session/providers/dumb_jump.rs:9:8:        let cwd = msg.get_cwd();";
        println!("{:?}", extract_jump_line_info(line));
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

        let line = r#"sorted_dict:18                 [variable@crates/icon/update_constants.py] sorted_dict = {k: disordered[k] for k in sorted(disordered)}"#;
        assert_eq!(
            (18, "crates/icon/update_constants.py"),
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
