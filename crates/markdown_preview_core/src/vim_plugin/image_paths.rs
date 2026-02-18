//! Image path rewriting utilities for vim-plugin mode.
//!
//! When serving markdown preview via WebSocket, relative image paths need
//! to be rewritten to use the local file server endpoint.

use regex::Regex;

/// Rewrites relative image paths in HTML to use a specified base path prefix.
///
/// Converts `<img src="path/to/image.png">` to `<img src="{prefix}/path/to/image.png">`
/// for relative paths only (absolute paths and URLs are left unchanged).
///
/// # Arguments
///
/// * `html` - The HTML content with image tags
/// * `prefix` - The prefix to add to relative image paths (e.g., "/files")
///
/// # Example
///
/// ```
/// use markdown_preview_core::vim_plugin::image_paths::rewrite_image_paths;
///
/// let html = r#"<img src="images/test.png">"#;
/// let result = rewrite_image_paths(html, "/files");
/// assert!(result.contains("/files/"));
/// ```
pub fn rewrite_image_paths(html: &str, prefix: &str) -> String {
    // Regex to match img tags with src attribute
    let img_regex = Regex::new(r#"<img\s+([^>]*?)src="([^"]+)"([^>]*)>"#).unwrap();

    img_regex
        .replace_all(html, |caps: &regex::Captures| {
            let before = &caps[1];
            let src = &caps[2];
            let after = &caps[3];

            // Skip absolute URLs (http://, https://, data:, //)
            if src.starts_with("http://")
                || src.starts_with("https://")
                || src.starts_with("data:")
                || src.starts_with("//")
                || src.starts_with('/')
            {
                return caps[0].to_string();
            }

            // URL-encode the path for safe transmission
            let encoded_src =
                percent_encoding::utf8_percent_encode(src, percent_encoding::NON_ALPHANUMERIC)
                    .to_string();

            format!(r#"<img {before}src="{prefix}/{encoded_src}"{after}>"#)
        })
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rewrite_relative_paths() {
        let html = r#"<img src="images/test.png">"#;
        let result = rewrite_image_paths(html, "/files");
        assert!(result.contains("/files/"));
    }

    #[test]
    fn test_preserve_absolute_urls() {
        let html = r#"<img src="https://example.com/image.png">"#;
        let result = rewrite_image_paths(html, "/files");
        assert_eq!(result, html);
    }

    #[test]
    fn test_preserve_data_urls() {
        let html = r#"<img src="data:image/png;base64,abc123">"#;
        let result = rewrite_image_paths(html, "/files");
        assert_eq!(result, html);
    }

    #[test]
    fn test_preserve_absolute_paths() {
        let html = r#"<img src="/absolute/path/image.png">"#;
        let result = rewrite_image_paths(html, "/files");
        assert_eq!(result, html);
    }

    #[test]
    fn test_preserve_protocol_relative_urls() {
        let html = r#"<img src="//cdn.example.com/image.png">"#;
        let result = rewrite_image_paths(html, "/files");
        assert_eq!(result, html);
    }
}
