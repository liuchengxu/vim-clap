//! Git repository utilities.

use std::path::Path;

/// Find the git repository root by walking up the directory tree.
///
/// # Arguments
///
/// * `path` - Path to a file or directory within a git repository
///
/// # Returns
///
/// The path to the git repository root, or `None` if not in a git repository.
///
/// # Example
///
/// ```
/// use markdown_preview_core::common::git::find_git_root;
///
/// // Returns Some("/path/to/repo") if file is in a git repo
/// let root = find_git_root("/path/to/repo/docs/readme.md");
/// ```
pub fn find_git_root(path: &str) -> Option<String> {
    let path = Path::new(path);
    let mut current = path.parent()?;

    loop {
        let git_dir = current.join(".git");
        if git_dir.exists() {
            return current.to_str().map(String::from);
        }

        current = current.parent()?;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_find_git_root_in_repo() {
        // This test file itself should be in a git repository
        let current_file = file!();
        let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
        let test_path = format!("{manifest_dir}/{current_file}");

        let result = find_git_root(&test_path);
        assert!(result.is_some(), "Should find git root for test file");

        let root = result.unwrap();
        assert!(
            Path::new(&root).join(".git").exists(),
            "Git root should contain .git directory"
        );
    }

    #[test]
    fn test_find_git_root_not_in_repo() {
        // Root directory is unlikely to be a git repo
        let result = find_git_root("/tmp/nonexistent/file.md");
        assert!(result.is_none(), "Should not find git root for /tmp path");
    }
}
