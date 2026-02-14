//! Embedded web assets for the markdown preview.
//!
//! This module provides access to the HTML, CSS, and JavaScript files
//! needed for the preview UI.
//!
//! The JavaScript is split into modules:
//! - `core.js`: Shared UI functionality (TOC, themes, fuzzy finder, etc.)
//! - `websocket-app.js`: WebSocket communication for vim-clap mode
//! - `tauri-app.js`: Tauri IPC for standalone app mode

/// HTML template with placeholders for CSS and JS.
pub const HTML_TEMPLATE: &str = include_str!("../js/index.html");

/// Main CSS styles.
pub const STYLES_CSS: &str = include_str!("../js/styles.css");

/// Theme CSS styles.
pub const THEMES_CSS: &str = include_str!("../js/themes.css");

/// Core JavaScript - shared UI functionality.
pub const CORE_JS: &str = include_str!("../js/core.js");

/// WebSocket JavaScript - vim-clap mode communication.
pub const WEBSOCKET_APP_JS: &str = include_str!("../js/websocket-app.js");

/// Tauri JavaScript - standalone app mode communication.
pub const TAURI_APP_JS: &str = include_str!("../js/tauri-app.js");

/// Options for building the HTML page.
#[derive(Debug, Clone, Default)]
pub struct AssetOptions {
    /// If true, include Tauri-specific JavaScript.
    pub tauri: bool,
}

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

    /// Get the WebSocket JavaScript (vim-clap mode).
    pub fn websocket_app_js() -> &'static str {
        WEBSOCKET_APP_JS
    }

    /// Get the Tauri JavaScript (standalone app mode).
    pub fn tauri_app_js() -> &'static str {
        TAURI_APP_JS
    }

    /// Build the complete HTML page with inlined CSS and JS.
    ///
    /// This replaces placeholder comments in the template with actual content:
    /// - `/*__STYLES_CSS__*/` -> styles.css content
    /// - `/*__THEMES_CSS__*/` -> themes.css content
    /// - `/*__APP_JS__*/` -> combined JavaScript content
    ///
    /// The JavaScript is built from modular files:
    /// - For WebSocket mode (default): core.js + websocket-app.js
    /// - For Tauri mode: core.js + tauri-app.js
    pub fn build_html(options: &AssetOptions) -> String {
        let mut html = HTML_TEMPLATE.to_string();

        // Inline CSS
        html = html.replace("/*__STYLES_CSS__*/", STYLES_CSS);
        html = html.replace("/*__THEMES_CSS__*/", THEMES_CSS);

        // Build JavaScript based on mode
        let js = if options.tauri {
            // Tauri mode: core + tauri-app
            format!("{CORE_JS}\n\n{TAURI_APP_JS}")
        } else {
            // WebSocket mode: core + websocket-app
            format!("{CORE_JS}\n\n{WEBSOCKET_APP_JS}")
        };

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
        assert!(!THEMES_CSS.is_empty());
    }

    #[test]
    fn test_js_modules_exist() {
        assert!(!CORE_JS.is_empty());
        assert!(!WEBSOCKET_APP_JS.is_empty());
        assert!(!TAURI_APP_JS.is_empty());
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
    fn test_tauri_js_has_required_functions() {
        // Tauri mode should have Tauri IPC
        assert!(TAURI_APP_JS.contains("__TAURI__"));
        assert!(TAURI_APP_JS.contains("invoke"));
        assert!(TAURI_APP_JS.contains("initCoreUI"));
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

        // Should NOT contain Tauri-specific code (outside of __TAURI__ check)
        // The tauri-app.js checks for __TAURI__ and returns early if not present
    }

    #[test]
    fn test_build_html_tauri_mode() {
        let html = Assets::build_html(&AssetOptions {
            tauri: true,
            ..Default::default()
        });

        // Should contain core and Tauri code
        assert!(html.contains("function initCoreUI"));
        assert!(html.contains("__TAURI__"));
        assert!(html.contains("invoke"));

        // Should NOT contain WebSocket code
        assert!(!html.contains("new WebSocket"));
    }
}
