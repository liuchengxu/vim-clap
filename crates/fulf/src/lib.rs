//! **Fu**zzy **l**inesearcher and **f**ilterer.
//!
//! Like regex searcher, but not regex searcher.

mod fileworks;
mod scoring_utils;
mod threadworks;

pub mod ascii;
pub mod utf8;

mod interface;
pub use interface::*;

pub use ignore::{Walk, WalkBuilder};
