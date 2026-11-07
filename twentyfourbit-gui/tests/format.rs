//! Behavioural tests for the display formatting helpers.
//!
//! These are the strings a user actually reads — a wrong duration or size is a
//! visible bug, so they are pinned against hand-computed expectations.

use std::time::Duration;
use twentyfourbit_gui::format::{format_bytes, format_duration, format_progress};

#[test]
fn durations_read_like_a_music_player() {
    assert_eq!(format_duration(Duration::ZERO), "0:00");
    assert_eq!(format_duration(Duration::from_secs(5)), "0:05");
    assert_eq!(format_duration(Duration::from_secs(59)), "0:59");
    assert_eq!(format_duration(Duration::from_secs(60)), "1:00");
    assert_eq!(format_duration(Duration::from_secs(61)), "1:01");
    assert_eq!(format_duration(Duration::from_secs(259)), "4:19");
    assert_eq!(format_duration(Duration::from_secs(3599)), "59:59");
    // Past an hour, hours appear; minutes and seconds are zero-padded.
    assert_eq!(format_duration(Duration::from_secs(3600)), "1:00:00");
    assert_eq!(format_duration(Duration::from_secs(3661)), "1:01:01");
}

#[test]
fn sub_second_durations_truncate_rather_than_round_up() {
    // 1.9s of audio has not reached the 2 second mark yet.
    assert_eq!(format_duration(Duration::from_millis(1_900)), "0:01");
}

#[test]
fn byte_sizes_use_binary_units() {
    assert_eq!(format_bytes(0), "0 B");
    assert_eq!(format_bytes(512), "512 B");
    assert_eq!(format_bytes(1024), "1.0 KB");
    assert_eq!(format_bytes(1536), "1.5 KB");
    assert_eq!(format_bytes(156_129_336), "148.9 MB");
    assert_eq!(format_bytes(5 * 1024 * 1024 * 1024), "5.0 GB");
}

#[test]
fn byte_sizes_never_show_a_bare_decimal_point() {
    // 1023 bytes must stay in bytes, not become "0.999 KB".
    assert_eq!(format_bytes(1023), "1023 B");
}

#[test]
fn progress_renders_a_percentage_clamped_to_0_100() {
    assert_eq!(format_progress(0.0), "0%");
    assert_eq!(format_progress(0.5), "50%");
    assert_eq!(format_progress(1.0), "100%");
    assert_eq!(format_progress(-1.0), "0%");
    assert_eq!(format_progress(2.0), "100%");
}

#[test]
fn unknown_progress_is_an_empty_string_not_a_guess() {
    // Downloading without a content-length gives the UI nothing to show.
    assert_eq!(format_progress(f64::NAN), "");
}
