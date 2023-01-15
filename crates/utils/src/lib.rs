//! Various utils functions for caching and file management.

use std::collections::hash_map::DefaultHasher;
use std::ffi::OsStr;
use std::hash::{Hash, Hasher};
use std::path::Path;
use std::process::{Command, Output};

pub mod bytelines;
mod io;

pub use self::io::{
    count_lines, create_or_overwrite, read_first_lines, read_lines, read_lines_from,
    remove_dir_contents,
};

/// Returns the width of displaying `n` on the screen.
///
/// Same with `n.to_string().len()` but without allocation.
pub fn display_width(n: usize) -> usize {
    if n == 0 {
        return 1;
    }

    let mut n = n;
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

pub fn calculate_hash<T: Hash>(t: &T) -> u64 {
    let mut s = DefaultHasher::new();
    t.hash(&mut s);
    s.finish()
}

/// Converts `shell_cmd` to `Command` with optional working directory.
pub fn as_std_command<P: AsRef<Path>>(shell_cmd: impl AsRef<OsStr>, dir: Option<P>) -> Command {
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
pub fn execute_at<S, P>(shell_cmd: S, dir: Option<P>) -> std::io::Result<Output>
where
    S: AsRef<OsStr>,
    P: AsRef<Path>,
{
    let mut cmd = as_std_command(shell_cmd, dir);
    cmd.output()
}
