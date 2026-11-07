//! Behavioural tests for the transport state machine.
//!
//! The GUI reads this state every frame and drives it with button presses, so
//! every transition has to be total: no state may leave the position or phase
//! inconsistent, and nothing may panic on out-of-range seeks.

use std::time::Duration;
use twentyfourbit_player::state::{Phase, Transport};

fn transport(seconds: u64) -> Transport {
    Transport::new(Duration::from_secs(seconds), 48_000, 2)
}

#[test]
fn starts_idle_at_zero() {
    let t = transport(200);
    assert_eq!(t.phase(), Phase::Idle);
    assert_eq!(t.position(), Duration::ZERO);
    assert_eq!(t.volume(), 1.0);
}

#[test]
fn play_pause_resume_cycles() {
    let mut t = transport(200);

    t.play();
    assert_eq!(t.phase(), Phase::Playing);

    t.pause();
    assert_eq!(t.phase(), Phase::Paused);

    t.resume();
    assert_eq!(t.phase(), Phase::Playing);
}

#[test]
fn pause_freezes_the_position() {
    let mut t = transport(200);
    t.play();
    t.advance_frames(48_000); // one second
    assert_eq!(t.position(), Duration::from_secs(1));

    t.pause();
    t.advance_frames(48_000); // must be ignored while paused
    assert_eq!(
        t.position(),
        Duration::from_secs(1),
        "a paused transport must not advance"
    );
}

#[test]
fn resume_continues_and_advances_again() {
    let mut t = transport(200);
    t.play();
    t.advance_frames(24_000); // 0.5s
    t.pause();
    t.advance_frames(24_000);
    t.resume();
    t.advance_frames(24_000); // 0.5s more

    assert_eq!(t.position(), Duration::from_secs(1));
}

#[test]
fn stop_resets_to_the_beginning() {
    let mut t = transport(200);
    t.play();
    t.advance_frames(96_000);
    t.stop();

    assert_eq!(t.phase(), Phase::Idle);
    assert_eq!(t.position(), Duration::ZERO);
}

#[test]
fn reaching_the_end_finishes_playback() {
    let mut t = transport(2);
    t.play();
    t.advance_frames(96_001); // just past the end

    assert_eq!(t.phase(), Phase::Finished);
    assert_eq!(t.position(), Duration::from_secs(2), "position clamps to the end");
}

#[test]
fn seek_clamps_to_the_media_bounds() {
    let mut t = transport(10);

    t.seek(Duration::from_secs(5));
    assert_eq!(t.position(), Duration::from_secs(5));

    t.seek(Duration::from_secs(999));
    assert_eq!(t.position(), Duration::from_secs(10), "cannot seek past the end");

    t.seek(Duration::ZERO);
    assert_eq!(t.position(), Duration::ZERO);
}

#[test]
fn seek_by_fraction_is_clamped() {
    let mut t = transport(10);
    t.seek_fraction(0.25);
    assert_eq!(t.position(), Duration::from_millis(2_500));

    t.seek_fraction(1.5);
    assert_eq!(t.position(), Duration::from_secs(10));
}

#[test]
fn volume_is_clamped_to_the_unit_interval() {
    let mut t = transport(10);
    t.set_volume(0.5);
    assert_eq!(t.volume(), 0.5);

    t.set_volume(-1.0);
    assert_eq!(t.volume(), 0.0);

    t.set_volume(3.0);
    assert_eq!(t.volume(), 1.0);
}

#[test]
fn progress_is_reported_as_a_fraction_including_the_empty_media_case() {
    let mut t = transport(10);
    t.play();
    t.advance_frames(24_000);
    assert!((t.progress() - 0.05).abs() < 1e-9);

    // Zero-length media must not divide by zero.
    let empty = Transport::new(Duration::ZERO, 48_000, 2);
    assert_eq!(empty.progress(), 0.0);
}

#[test]
fn a_paused_transport_still_accepts_a_new_position() {
    let mut t = transport(100);
    t.play();
    t.pause();
    t.seek(Duration::from_secs(42));

    assert_eq!(t.position(), Duration::from_secs(42));
    assert_eq!(t.phase(), Phase::Paused, "seeking must not change the phase");
}
