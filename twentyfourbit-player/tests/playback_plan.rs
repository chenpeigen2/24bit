//! Behavioural tests for the playback seam.
//!
//! Audio output hardware is not available in CI, so the *device-independent*
//! logic is what these tests pin: which sample rate we should ask the device
//! for, and how the resampling plan is derived. Everything here is pure and
//! testable without a sound card.

use twentyfourbit_player::{PlaybackPlan, ResampleMode};

/// The site's master tier is 24bit/192kHz. Most consumer output devices cap out
/// far lower, so the player must plan a downsample rather than fail.
#[test]
fn master_192k_plans_a_downsample_for_common_device() {
    let plan = PlaybackPlan::for_source(192_000, 48_000);
    assert_eq!(plan.source_rate, 192_000);
    assert_eq!(plan.output_rate, 48_000);
    assert_eq!(plan.mode, ResampleMode::Downsample);
    assert_eq!(plan.ratio(), 0.25);
}

/// A device that already supports the file's rate is played untouched.
#[test]
fn matching_rate_needs_no_resampling() {
    let plan = PlaybackPlan::for_source(96_000, 96_000);
    assert_eq!(plan.mode, ResampleMode::Passthrough);
    assert_eq!(plan.ratio(), 1.0);
}

/// A device faster than the source is upsampled rather than refused.
#[test]
fn faster_device_plans_an_upsample() {
    let plan = PlaybackPlan::for_source(44_100, 192_000);
    assert_eq!(plan.mode, ResampleMode::Upsample);
    assert!((plan.ratio() - 4.3537).abs() < 0.001, "ratio was {}", plan.ratio());
}

/// When the device refuses to report a rate, we fall back to 48kHz — the
/// universally safe default — instead of panicking.
#[test]
fn unknown_device_rate_falls_back_to_safe_default() {
    let plan = PlaybackPlan::for_source(192_000, 0);
    assert_eq!(plan.output_rate, 48_000);
    assert_eq!(plan.mode, ResampleMode::Downsample);
}

/// The plan reports the effective bit depth it will play at, so the CLI can
/// tell the user what they are actually hearing.
#[test]
fn plan_reports_effective_sample_count() {
    let plan = PlaybackPlan::for_source(192_000, 48_000);
    // 4x fewer frames at the output rate.
    assert_eq!(plan.effective_frames(4_970_928), 1_242_732);
}

/// Truncating rather than rounding would silently drop a frame; the plan must
/// never return fewer frames than the maths allows.
#[test]
fn effective_frames_rounds_to_nearest() {
    let plan = PlaybackPlan::for_source(192_000, 44_100);
    // 1000 * (44100/192000) = 229.6875 -> 230
    assert_eq!(plan.effective_frames(1_000), 230);
}

/// A source rate of zero is impossible but must not divide by zero.
#[test]
fn zero_source_rate_is_handled() {
    let plan = PlaybackPlan::for_source(0, 48_000);
    assert_eq!(plan.mode, ResampleMode::Passthrough);
    assert_eq!(plan.output_rate, 48_000);
}
