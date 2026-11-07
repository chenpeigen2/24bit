//! Playback support for 24bit.net downloads.
//!
//! Split out from `twentyfourbit-core` so that download-only callers never link
//! an audio stack.
//!
//! Two things make this non-trivial:
//!
//! * The master tier is 24bit/192kHz, which most output devices cannot accept.
//!   [`PlaybackPlan`] decides the output rate and whether to resample *before*
//!   any hardware is opened, and that decision is pure and unit-tested.
//! * Decoding happens fully before playback starts, which keeps the pipeline
//!   simple and avoids dropouts on 150MB master files.

pub mod engine;
pub mod plan;
pub mod testing;

pub use engine::{play_file, PlayError};
pub use plan::{PlaybackPlan, ResampleMode, SAFE_DEFAULT_RATE};
