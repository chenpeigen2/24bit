//! Test-only hooks.
//!
//! Not part of the public contract; gated so application code cannot depend on
//! it by accident.

use std::sync::atomic::AtomicUsize;
use std::sync::Arc;

/// Play raw interleaved samples through the real output stage.
pub fn play_samples(samples: &[f32], sample_rate: u32, _channels: u16) -> Result<(), crate::PlayError> {
    crate::engine::output(samples, sample_rate)
}

/// Like [`play_samples`], but reports how many samples the device consumed.
pub fn play_samples_counted(
    samples: &[f32],
    sample_rate: u32,
    _channels: u16,
    consumed: Arc<AtomicUsize>,
) -> Result<(), crate::PlayError> {
    crate::engine::output_counted(samples, sample_rate, consumed)
}
