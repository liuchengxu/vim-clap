//! This crate implements the functionality of downloading an asset from GitHub release
//! and provides the feature of upgrading the maple executable on top of it.

mod github;
mod maple_upgrade;

pub use maple_upgrade::Upgrade;
