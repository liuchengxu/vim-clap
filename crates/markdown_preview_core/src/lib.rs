//! Core library for markdown preview functionality.
//!
//! This crate provides reusable components for rendering markdown to HTML,
//! generating table of contents, calculating document statistics, watching
//! files for changes, and defining the communication protocol.
//!
//! # Modules
//!
//! - [`render`] - Markdown to HTML conversion with GitHub-style features
//! - [`toc`] - Table of contents generation
//! - [`stats`] - Document statistics calculation
//! - [`watcher`] - File watching abstraction
//! - [`protocol`] - Message types for client-server communication
//! - [`assets`] - Embedded web assets (HTML, CSS, JS)

pub mod assets;
pub mod protocol;
pub mod render;
pub mod stats;
pub mod toc;
pub mod watcher;

// Re-export commonly used types at crate root
pub use protocol::{find_git_root, ClientMessage, ServerMessage};
pub use render::{rewrite_image_paths, to_html, RenderOptions, RenderResult};
pub use stats::{calculate_document_stats, DocumentStats};
pub use toc::{find_toc_range, generate_toc, slugify, TocConfig};
pub use watcher::{FileWatcher, WatchEvent};

// Re-export frecency for path history tracking
pub use frecency;
