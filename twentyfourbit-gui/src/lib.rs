//! Desktop GUI for 24bit.net.
//!
//! The crate is split so that almost all of it is testable without a window:
//! [`backend`] does the network and cache work, [`format`] does the strings,
//! [`fonts`] finds a CJK face, and [`app`] only paints.

pub mod app;
pub mod backend;
pub mod covers;
pub mod fonts;
pub mod format;
pub mod theme;
