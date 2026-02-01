//! Core library for markdown preview functionality.
//!
//! This crate provides reusable components for rendering markdown to HTML,
//! generating table of contents, calculating document statistics, and watching
//! files for changes.
//!
//! # Features
//!
//! - `vim-plugin` - Enables vim-plugin specific code (protocol messages, image path rewriting)
//! - `gui` - Enables GUI-specific code (currently placeholder)
//! - `offline` - Enables offline bundled assets (KaTeX, highlight.js, Mermaid)
//!
//! # Modules
//!
//! - [`common`] - Common utilities shared across all modes (e.g., `find_git_root`)
//! - [`render`] - Markdown to HTML conversion with GitHub-style features
//! - [`toc`] - Table of contents generation
//! - [`stats`] - Document statistics calculation
//! - [`watcher`] - File watching abstraction
//! - [`assets`] - Embedded web assets (HTML, CSS, JS)
//! - [`vim_plugin`] - Vim-plugin specific code (requires `vim-plugin` feature)

pub mod assets;
pub mod common;
pub mod document;
pub mod render;
pub mod stats;
pub mod toc;
pub mod watcher;

#[cfg(feature = "vim-plugin")]
pub mod vim_plugin;

// Re-export commonly used types at crate root
pub use common::git::find_git_root;
pub use document::DocumentType;
pub use render::{to_html, PreviewMode, RenderOptions, RenderOutput, RenderResult};
pub use stats::{calculate_document_stats, calculate_pdf_stats, DocumentStats};
pub use toc::{find_toc_range, generate_toc, slugify, TocConfig};
pub use watcher::{FileWatcher, WatchEvent};

// Re-export frecency for path history tracking
pub use frecency;
