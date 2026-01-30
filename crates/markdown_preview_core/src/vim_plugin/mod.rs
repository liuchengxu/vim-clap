//! Vim-plugin specific functionality.
//!
//! This module contains code that is only used by the vim-clap plugin integration,
//! including:
//!
//! - Protocol message types for WebSocket communication
//! - Image path rewriting for serving local images
//!
//! This module is feature-gated behind the `vim-plugin` feature.

pub mod image_paths;
pub mod protocol;
