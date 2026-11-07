//! Behavioural tests for the audio output stage.
//!
//! These run against the real audio host but do not require human listening:
//! the contract is *timing*. Feeding exactly one second of samples must block
//! for approximately one second, because the output callback consumes samples
//! at the device's pace, not ours.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Feeding one second of silence at 48kHz to the output stage must block for
/// close to one second, not return immediately.
#[test]
fn output_blocks_for_the_duration_of_the_audio() {
    let rate = 48_000u32;
    let channels = 2u16;
    let samples = vec![0.0f32; rate as usize * channels as usize]; // exactly 1s

    let t = Instant::now();
    // `output` is private to the engine; the public seam is `play_file`, but a
    // real file decode would take longer than the audio itself at these sizes.
    // Instead go through the crate's test-only hook.
    twentyfourbit_player::testing::play_samples(&samples, rate, channels).expect("output");
    let elapsed = t.elapsed();

    assert!(
        elapsed >= Duration::from_millis(900),
        "1s of audio returned after {elapsed:?} — output is not consuming at device pace"
    );
    assert!(
        elapsed < Duration::from_millis(3_000),
        "1s of audio took {elapsed:?} — output is stalled"
    );
}

/// The completion flag must only be raised once every sample has been handed to
/// the device — not at the end of the *first* callback, which is the bug this
/// suite exists to pin.
#[test]
fn every_sample_is_consumed_before_completion() {
    let rate = 48_000u32;
    let channels = 2u16;
    // 0.5s of audio; the device callback period is typically ~10ms, so this
    // takes dozens of callbacks.
    let samples = vec![0.25f32; rate as usize / 2 * channels as usize];

    let consumed = Arc::new(AtomicUsize::new(0));
    twentyfourbit_player::testing::play_samples_counted(&samples, rate, channels, consumed.clone())
        .expect("output");

    assert_eq!(
        consumed.load(Ordering::SeqCst),
        samples.len(),
        "every sample must be consumed exactly once"
    );
}
