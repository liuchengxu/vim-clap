//! **Fu**zzy **l**inesearcher and **f**ilterer.
//!
//! Like regex searcher, but not regex searcher.

pub mod ascii;
pub mod fileworks;
pub mod threadworks;
pub mod utf8;

mod scoring_utils;

mod renamemeplsmynamesucks;
pub use renamemeplsmynamesucks::*;

pub use ignore::{Walk, WalkBuilder};
