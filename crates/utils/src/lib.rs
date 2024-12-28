//! Various utils functions for caching and file management.

use std::collections::hash_map::DefaultHasher;
use std::ffi::OsStr;
use std::hash::{Hash, Hasher};
use std::path::Path;
use std::process::{Command, Output};

pub mod bytelines;
pub mod io;

/// Returns the width of displaying `n` on the screen.
///
/// Same with `n.to_string().len()` but without allocation.
pub fn display_width(mut n: usize) -> usize {
    if n == 0 {
        return 1;
    }

    let mut len = 0;
    while n > 0 {
        len += 1;
        n /= 10;
    }

    len
}

/// Returns true if `dir` is a git repo, including git submodule.
pub fn is_git_repo(dir: &Path) -> bool {
    dir.join(".git").exists()
}

pub fn compute_hash<T: Hash>(t: &T) -> u64 {
    let mut s = DefaultHasher::new();
    t.hash(&mut s);
    s.finish()
}

/// Constructs a `Command` for executing a shell command.
pub fn build_shell_command<P: AsRef<Path>>(
    shell_cmd: impl AsRef<OsStr>,
    dir: Option<P>,
) -> Command {
    let mut cmd = if cfg!(target_os = "windows") {
        let mut cmd = Command::new("cmd");
        cmd.arg("/C").arg(shell_cmd.as_ref());
        cmd
    } else {
        let mut cmd = Command::new("bash");
        cmd.arg("-c").arg(shell_cmd.as_ref());
        cmd
    };

    if let Some(d) = dir {
        cmd.current_dir(d);
    }

    cmd
}

/// Executes the `shell_cmd` and returns the output.
pub fn execute_shell_command<S, P>(shell_cmd: S, dir: Option<P>) -> std::io::Result<Output>
where
    S: AsRef<OsStr>,
    P: AsRef<Path>,
{
    let mut cmd = build_shell_command(shell_cmd, dir);
    cmd.output()
}

/// Converts the char positions to byte positions for use in Vim/NeoVim.
///
/// Vim and Neovim highlights use byte positions, this utility translate char positions.
pub fn char_indices_to_byte_indices(s: &str, char_indices: &[usize]) -> Vec<usize> {
    s.char_indices()
        .enumerate()
        .filter_map(|(char_idx, (byte_idx, _char))| {
            if char_indices.contains(&char_idx) {
                Some(byte_idx)
            } else {
                None
            }
        })
        .collect::<Vec<_>>()
}

/// Returns the char index of given byte index (0-based) in a line.
pub fn char_index_at_byte(line: &str, byte_idx: usize) -> Option<usize> {
    line.char_indices().enumerate().find_map(
        |(c_idx, (b_idx, _c))| {
            if byte_idx == b_idx {
                Some(c_idx)
            } else {
                None
            }
        },
    )
}

/// Returns the char at given byte index (0-based) in a line.
pub fn char_at_byte(line: &str, byte_idx: usize) -> Option<char> {
    line.char_indices()
        .find_map(|(b_idx, c)| if byte_idx == b_idx { Some(c) } else { None })
}
