//! Embedded web assets for the markdown preview.
//!
//! This module provides access to the HTML, CSS, and JavaScript files
//! needed for the preview UI.
//!
//! The JavaScript is split into modules:
//! - `core.js`: Shared UI functionality (TOC, themes, etc.)
//! - `tooltips.js`: Symbol, link, and path hover tooltips
//! - `search.js`: Fuzzy finder for searching headings and full text
//! - `diff.js`: Diff overlay for showing changes since last view
//! - `websocket-app.js`: WebSocket communication for vim-clap mode

/// HTML template with placeholders for CSS and JS.
pub const HTML_TEMPLATE: &str = include_str!("../js/index.html");

/// Main CSS styles.
pub const STYLES_CSS: &str = include_str!("../js/styles.css");

/// Fuzzy finder CSS styles.
pub const FUZZY_FINDER_CSS: &str = include_str!("../js/fuzzy-finder.css");

/// Presentation mode CSS styles.
pub const PRESENTATION_CSS: &str = include_str!("../js/presentation.css");

/// Diff overlay CSS styles.
pub const DIFF_CSS: &str = include_str!("../js/diff.css");

/// Theme CSS styles.
pub const THEMES_CSS: &str = include_str!("../js/themes.css");

/// Core JavaScript - shared UI functionality.
pub const CORE_JS: &str = include_str!("../js/core.js");

/// Tooltips JavaScript - symbol, link, and path hover tooltips.
pub const TOOLTIPS_JS: &str = include_str!("../js/tooltips.js");

/// Search JavaScript - fuzzy finder for headings and full text.
pub const SEARCH_JS: &str = include_str!("../js/search.js");

/// Diff JavaScript - diff overlay for showing changes since last view.
pub const DIFF_JS: &str = include_str!("../js/diff.js");

/// WebSocket JavaScript - vim-clap mode communication.
pub const WEBSOCKET_APP_JS: &str = include_str!("../js/websocket-app.js");

/// Options for building the HTML page.
#[derive(Debug, Clone, Default)]
pub struct AssetOptions;

/// Provides access to embedded assets.
pub struct Assets;

impl Assets {
    /// Get the raw HTML template.
    pub fn html_template() -> &'static str {
        HTML_TEMPLATE
    }

    /// Get the CSS styles.
    pub fn styles_css() -> &'static str {
        STYLES_CSS
    }

    /// Get the theme CSS.
    pub fn themes_css() -> &'static str {
        THEMES_CSS
    }

    /// Get the core JavaScript (shared UI functionality).
    pub fn core_js() -> &'static str {
        CORE_JS
    }

    /// Get the tooltips JavaScript (symbol, link, and path tooltips).
    pub fn tooltips_js() -> &'static str {
        TOOLTIPS_JS
    }

    /// Get the search JavaScript (fuzzy finder).
    pub fn search_js() -> &'static str {
        SEARCH_JS
    }

    /// Get the diff JavaScript (diff overlay).
    pub fn diff_js() -> &'static str {
        DIFF_JS
    }

    /// Get the WebSocket JavaScript (vim-clap mode).
    pub fn websocket_app_js() -> &'static str {
        WEBSOCKET_APP_JS
    }

    /// Build the complete HTML page with inlined CSS and JS.
    ///
    /// This replaces placeholder comments in the template with actual content:
    /// - `/*__STYLES_CSS__*/` -> styles.css content
    /// - `/*__THEMES_CSS__*/` -> themes.css content
    /// - `/*__APP_JS__*/` -> combined JavaScript content
    ///
    /// JavaScript modules: core.js + tooltips.js + search.js + diff.js + websocket-app.js
    pub fn build_html(_options: &AssetOptions) -> String {
        let mut html = HTML_TEMPLATE.to_string();

        // Inline CSS: base styles + feature styles + themes
        let css = format!(
            "{STYLES_CSS}\n\n{FUZZY_FINDER_CSS}\n\n{PRESENTATION_CSS}\n\n{DIFF_CSS}\n\n{THEMES_CSS}"
        );
        html = html.replace("/*__STYLES_CSS__*/", &css);
        html = html.replace("/*__THEMES_CSS__*/", "");

        // Build JavaScript: core + tooltips + search + diff + websocket-app
        let js =
            format!("{CORE_JS}\n\n{TOOLTIPS_JS}\n\n{SEARCH_JS}\n\n{DIFF_JS}\n\n{WEBSOCKET_APP_JS}");

        html = html.replace("/*__APP_JS__*/", &js);

        html
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_html_template_exists() {
        assert!(!HTML_TEMPLATE.is_empty());
        assert!(HTML_TEMPLATE.contains("<!DOCTYPE html>"));
    }

    #[test]
    fn test_styles_exist() {
        assert!(!STYLES_CSS.is_empty());
        assert!(!FUZZY_FINDER_CSS.is_empty());
        assert!(!PRESENTATION_CSS.is_empty());
        assert!(!DIFF_CSS.is_empty());
        assert!(!THEMES_CSS.is_empty());
    }

    #[test]
    fn test_js_modules_exist() {
        assert!(!CORE_JS.is_empty());
        assert!(!TOOLTIPS_JS.is_empty());
        assert!(!SEARCH_JS.is_empty());
        assert!(!DIFF_JS.is_empty());
        assert!(!WEBSOCKET_APP_JS.is_empty());
    }

    #[test]
    fn test_core_js_has_required_functions() {
        // Core should have shared UI functions
        assert!(CORE_JS.contains("function codeHighlight"));
        assert!(CORE_JS.contains("function generateTOC"));
        assert!(CORE_JS.contains("function changeTheme"));
        assert!(CORE_JS.contains("function initCoreUI"));
    }

    #[test]
    fn test_websocket_js_has_required_functions() {
        // WebSocket mode should have WebSocket connection
        assert!(WEBSOCKET_APP_JS.contains("WebSocket"));
        assert!(WEBSOCKET_APP_JS.contains("initCoreUI"));
    }

    #[test]
    fn test_build_html_websocket_mode() {
        let html = Assets::build_html(&AssetOptions::default());

        // Should have replaced placeholders
        assert!(!html.contains("/*__STYLES_CSS__*/"));
        assert!(!html.contains("/*__THEMES_CSS__*/"));
        assert!(!html.contains("/*__APP_JS__*/"));

        // Should contain core and websocket code
        assert!(html.contains("function initCoreUI"));
        assert!(html.contains("WebSocket"));
    }
}
