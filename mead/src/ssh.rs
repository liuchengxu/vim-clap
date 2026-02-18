//! SSH command helpers for reading remote files.
//!
//! Shells out to the system `ssh` binary so we inherit `~/.ssh/config`, keys,
//! and agent — zero setup on the remote machine.

use tokio::process::Command;

/// Parsed SCP-style SSH path: `[user@]host:/absolute/path`.
#[derive(Debug, Clone)]
pub struct SshTarget {
    pub user: Option<String>,
    pub host: String,
    pub path: String,
}

// ========================================
// Input validation
// ========================================

/// Allowed chars for SSH user/host: must start with alphanumeric, then
/// alphanumeric/underscore/dash/dot. Rejects leading `-` to prevent
/// option injection into ssh.
fn is_valid_identifier(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }
    let mut chars = s.chars();
    let first = chars.next().expect("non-empty string");
    first.is_ascii_alphanumeric()
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_')
}

fn validate_user(user: &str) -> Result<(), String> {
    if is_valid_identifier(user) {
        Ok(())
    } else {
        Err(format!("Invalid SSH user: {user}"))
    }
}

fn validate_host(host: &str) -> Result<(), String> {
    if is_valid_identifier(host) {
        Ok(())
    } else {
        Err(format!("Invalid SSH host: {host}"))
    }
}

fn validate_path(path: &str) -> Result<(), String> {
    if !path.starts_with('/') {
        return Err("Remote path must be absolute (start with /)".to_string());
    }
    if path.contains('\0') {
        return Err("Remote path must not contain null bytes".to_string());
    }
    if path.contains('\n') {
        return Err("Remote path must not contain newlines".to_string());
    }
    Ok(())
}

// ========================================
// Shell escaping
// ========================================

/// Shell-escape a string for safe use in a remote SSH command.
/// Wraps in single quotes, escaping any embedded single quotes.
fn shell_escape(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

// ========================================
// SSH command builder
// ========================================

fn ssh_command(user: Option<&str>, host: &str) -> Result<Command, String> {
    validate_host(host)?;
    if let Some(u) = user {
        validate_user(u)?;
    }

    let mut cmd = Command::new("ssh");
    cmd.arg("-o")
        .arg("BatchMode=yes")
        .arg("-o")
        .arg("ConnectTimeout=10")
        .arg("-o")
        .arg("StrictHostKeyChecking=accept-new");

    let target = match user {
        Some(u) => format!("{u}@{host}"),
        None => host.to_string(),
    };
    cmd.arg("--").arg(target);
    Ok(cmd)
}

/// Classify SSH stderr output into a user-friendly error message.
fn classify_ssh_error(stderr: &str, exit_code: Option<i32>) -> String {
    let stderr_lower = stderr.to_lowercase();

    if stderr_lower.contains("permission denied") {
        return "SSH permission denied — check your key or credentials".to_string();
    }
    if stderr_lower.contains("connection refused") {
        return "SSH connection refused — is the SSH server running?".to_string();
    }
    if stderr_lower.contains("could not resolve hostname")
        || stderr_lower.contains("name or service not known")
    {
        return "SSH host not found — check the hostname".to_string();
    }
    if stderr_lower.contains("connection timed out") || stderr_lower.contains("timed out") {
        return "SSH connection timed out — host may be unreachable".to_string();
    }
    if stderr_lower.contains("no such file or directory") {
        return "File not found on remote host".to_string();
    }
    if stderr_lower.contains("host key verification failed") {
        return "SSH host key verification failed".to_string();
    }
    if stderr_lower.contains("no route to host") {
        return "SSH host unreachable — no route to host".to_string();
    }

    // Fallback: include exit code if available
    match exit_code {
        Some(255) => format!("SSH connection failed: {}", stderr.trim()),
        Some(code) => format!("Remote command failed (exit {code}): {}", stderr.trim()),
        None => format!("SSH command failed: {}", stderr.trim()),
    }
}

/// Run an SSH command and return stdout, or a classified error.
async fn run_ssh(mut cmd: Command) -> Result<String, String> {
    let output = cmd
        .output()
        .await
        .map_err(|e| format!("Failed to run ssh: {e}"))?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(classify_ssh_error(&stderr, output.status.code()))
    }
}

// ========================================
// Public remote operations
// ========================================

/// Read a file's contents from a remote host.
pub async fn read_file(user: Option<&str>, host: &str, path: &str) -> Result<String, String> {
    validate_path(path)?;
    let mut cmd = ssh_command(user, host)?;
    cmd.arg(format!("cat -- {}", shell_escape(path)));
    run_ssh(cmd).await
}

/// Get the modification time of a remote file (Unix timestamp in seconds).
///
/// Tries GNU `stat` first (Linux), then BSD `stat` (macOS).
pub async fn stat_mtime(user: Option<&str>, host: &str, path: &str) -> Result<u64, String> {
    validate_path(path)?;
    let mut cmd = ssh_command(user, host)?;
    // GNU stat first, BSD stat fallback
    cmd.arg(format!(
        "stat -c %Y -- {} 2>/dev/null || stat -f %m -- {} 2>/dev/null",
        shell_escape(path),
        shell_escape(path)
    ));

    let output = run_ssh(cmd).await?;
    let seconds: u64 = output
        .trim()
        .parse()
        .map_err(|e| format!("Failed to parse mtime: {e}"))?;

    // Convert seconds to milliseconds
    seconds
        .checked_mul(1000)
        .ok_or_else(|| "mtime overflow".to_string())
}

/// List directory entries on a remote host.
///
/// Returns entries with `/` appended to directories. Note: filenames containing
/// newlines will be misinterpreted, but markdown filenames never do in practice.
pub async fn list_dir(user: Option<&str>, host: &str, path: &str) -> Result<Vec<String>, String> {
    validate_path(path)?;
    let mut cmd = ssh_command(user, host)?;
    cmd.arg(format!("ls -1pA -- {}", shell_escape(path)));

    let output = run_ssh(cmd).await?;
    Ok(output
        .lines()
        .filter(|l| !l.is_empty())
        .map(String::from)
        .collect())
}

/// Get the git branch for a remote directory.
pub async fn git_branch(user: Option<&str>, host: &str, dir: &str) -> Option<String> {
    let mut cmd = ssh_command(user, host).ok()?;
    cmd.arg(format!(
        "git -C {} rev-parse --abbrev-ref HEAD",
        shell_escape(dir)
    ));
    run_ssh(cmd)
        .await
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Get the last commit author for a remote file.
pub async fn git_last_author(
    user: Option<&str>,
    host: &str,
    dir: &str,
    file_path: &str,
) -> Option<String> {
    let mut cmd = ssh_command(user, host).ok()?;
    cmd.arg(format!(
        "git -C {} log -1 --format=%an -- {}",
        shell_escape(dir),
        shell_escape(file_path)
    ));
    run_ssh(cmd)
        .await
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Get the remote URL of the origin remote for a git repo.
pub async fn git_remote_url(user: Option<&str>, host: &str, dir: &str) -> Option<String> {
    let mut cmd = ssh_command(user, host).ok()?;
    cmd.arg(format!(
        "git -C {} remote get-url origin",
        shell_escape(dir)
    ));
    run_ssh(cmd)
        .await
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Get the git root directory for a remote path.
pub async fn git_root(user: Option<&str>, host: &str, dir: &str) -> Option<String> {
    let mut cmd = ssh_command(user, host).ok()?;
    cmd.arg(format!(
        "git -C {} rev-parse --show-toplevel",
        shell_escape(dir)
    ));
    run_ssh(cmd)
        .await
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Derive a GitHub branch URL from a remote URL and branch name.
pub fn git_branch_url_from_remote(remote_url: &str, branch: &str) -> Option<String> {
    let github_base = if remote_url.starts_with("git@github.com:") {
        let path = remote_url.trim_start_matches("git@github.com:");
        let path = path.trim_end_matches(".git");
        format!("https://github.com/{path}")
    } else if remote_url.starts_with("https://github.com/") {
        remote_url.trim_end_matches(".git").to_string()
    } else {
        return None;
    };
    Some(format!("{github_base}/tree/{branch}"))
}

// ========================================
// Path parsing
// ========================================

/// Check if a string is an SSH path (SCP-style: `[user@]host:/absolute/path`).
pub fn is_ssh_path(input: &str) -> bool {
    parse_ssh_path(input).is_some()
}

/// Parse an SCP-style SSH path into its components.
///
/// Format: `[user@]host:/absolute/path`
///
/// Returns `None` if the input is not a valid SSH path.
pub fn parse_ssh_path(input: &str) -> Option<SshTarget> {
    // Must not be a URL
    if input.starts_with("http://") || input.starts_with("https://") {
        return None;
    }

    // Find the first `:` followed by `/`
    let colon_pos = input.find(":/")?;

    let user_host = &input[..colon_pos];
    let path = &input[colon_pos + 1..]; // includes leading /

    // Validate path
    if validate_path(path).is_err() {
        return None;
    }

    // Parse user@host or just host
    let (user, host) = if let Some(at_pos) = user_host.find('@') {
        let user = &user_host[..at_pos];
        let host = &user_host[at_pos + 1..];
        (Some(user), host)
    } else {
        (None, user_host)
    };

    // Validate host (must start with alphanumeric)
    if !is_valid_identifier(host) {
        return None;
    }

    // Validate user if present
    if let Some(u) = user {
        if !is_valid_identifier(u) {
            return None;
        }
    }

    Some(SshTarget {
        user: user.map(String::from),
        host: host.to_string(),
        path: path.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_ssh_path_with_user() {
        let target = parse_ssh_path("xlc@100.114.210.42:/home/xlc/docs/readme.md").unwrap();
        assert_eq!(target.user.as_deref(), Some("xlc"));
        assert_eq!(target.host, "100.114.210.42");
        assert_eq!(target.path, "/home/xlc/docs/readme.md");
    }

    #[test]
    fn test_parse_ssh_path_without_user() {
        let target = parse_ssh_path("myserver:/etc/motd").unwrap();
        assert!(target.user.is_none());
        assert_eq!(target.host, "myserver");
        assert_eq!(target.path, "/etc/motd");
    }

    #[test]
    fn test_parse_ssh_path_with_ssh_config_alias() {
        let target = parse_ssh_path("dev_box:/home/user/notes.md").unwrap();
        assert!(target.user.is_none());
        assert_eq!(target.host, "dev_box");
        assert_eq!(target.path, "/home/user/notes.md");
    }

    #[test]
    fn test_parse_ssh_path_rejects_urls() {
        assert!(parse_ssh_path("https://github.com/foo/bar").is_none());
        assert!(parse_ssh_path("http://localhost:8080/path").is_none());
    }

    #[test]
    fn test_parse_ssh_path_rejects_relative_path() {
        assert!(parse_ssh_path("host:relative/path").is_none());
    }

    #[test]
    fn test_parse_ssh_path_rejects_leading_dash_host() {
        assert!(parse_ssh_path("-evil:/etc/passwd").is_none());
    }

    #[test]
    fn test_parse_ssh_path_rejects_empty_host() {
        assert!(parse_ssh_path(":/etc/passwd").is_none());
    }

    #[test]
    fn test_is_ssh_path() {
        assert!(is_ssh_path("user@host:/path/file.md"));
        assert!(is_ssh_path("host:/path/file.md"));
        assert!(!is_ssh_path("/local/path/file.md"));
        assert!(!is_ssh_path("https://example.com"));
    }

    #[test]
    fn test_shell_escape() {
        assert_eq!(shell_escape("/simple/path"), "'/simple/path'");
        assert_eq!(
            shell_escape("/path/with spaces/file"),
            "'/path/with spaces/file'"
        );
        assert_eq!(shell_escape("/path/with'quote"), "'/path/with'\\''quote'");
    }

    #[test]
    fn test_validate_path() {
        assert!(validate_path("/valid/path").is_ok());
        assert!(validate_path("/path with spaces").is_ok());
        assert!(validate_path("relative/path").is_err());
        assert!(validate_path("/path\0with_null").is_err());
        assert!(validate_path("/path\nwith_newline").is_err());
    }

    #[test]
    fn test_git_branch_url_from_remote() {
        assert_eq!(
            git_branch_url_from_remote("git@github.com:user/repo.git", "main"),
            Some("https://github.com/user/repo/tree/main".to_string())
        );
        assert_eq!(
            git_branch_url_from_remote("https://github.com/user/repo.git", "develop"),
            Some("https://github.com/user/repo/tree/develop".to_string())
        );
        assert_eq!(
            git_branch_url_from_remote("git@gitlab.com:user/repo.git", "main"),
            None
        );
    }
}
