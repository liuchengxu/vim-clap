//! **Fu**zzy **l**inesearcher and **f**ilterer.
//!
//! Like regex searcher, but not regex searcher.

mod bytelines;

pub mod fzy_algo;

mod interface;
pub use interface::*;

pub use ignore::{Walk, WalkBuilder};
