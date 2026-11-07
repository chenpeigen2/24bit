//! Behavioural tests for the search debounce.
//!
//! Typing "富士山下" one character at a time must produce one search, not four.
//! The logic is pure (it takes a timestamp rather than reading a clock) so the
//! timing can be tested exactly.

use twentyfourbit_gui::debounce::Debounce;

#[test]
fn nothing_fires_before_an_edit() {
    let mut d = Debounce::new(0.3);
    assert!(!d.poll(100.0), "an untouched debounce never fires");
}

#[test]
fn a_single_edit_fires_exactly_once_after_the_delay() {
    let mut d = Debounce::new(0.3);
    d.note_edit(10.0);

    assert!(!d.poll(10.1), "too early");
    assert!(!d.poll(10.29), "still too early");
    assert!(d.poll(10.3), "due at the delay");
    assert!(!d.poll(10.4), "must not fire twice for one edit");
    assert!(!d.poll(99.0), "and never again");
}

#[test]
fn typing_quickly_collapses_into_one_firing() {
    let mut d = Debounce::new(0.3);

    // Four keystrokes 80ms apart — the classic "富士山下" case.
    for i in 0..4 {
        d.note_edit(10.0 + i as f64 * 0.08);
        assert!(
            !d.poll(10.0 + i as f64 * 0.08 + 0.1),
            "no firing while the user is still typing"
        );
    }

    // Last edit was at 10.24; the window closes 300ms later. Check just past
    // the boundary rather than exactly on it: timestamps are f64 and an exact
    // equality here would be measuring floating-point luck, not behaviour.
    assert!(d.poll(10.55), "fires once the typing settles");
    assert!(!d.poll(11.0), "only once");
}

#[test]
fn continued_typing_keeps_postponing() {
    let mut d = Debounce::new(0.3);
    d.note_edit(0.0);
    assert!(!d.poll(0.29));

    d.note_edit(0.29); // user typed again just in time
    assert!(!d.poll(0.30), "the window restarted");
    assert!(!d.poll(0.58));
    assert!(d.poll(0.59));
}

#[test]
fn submitting_explicitly_cancels_a_pending_debounce() {
    let mut d = Debounce::new(0.3);
    d.note_edit(1.0);

    // The user pressed Enter, which searches immediately.
    d.cancel();
    assert!(!d.poll(2.0), "an explicit submit must not be followed by a duplicate");
}

#[test]
fn a_zero_delay_fires_immediately_but_still_once() {
    let mut d = Debounce::new(0.0);
    d.note_edit(5.0);
    assert!(d.poll(5.0));
    assert!(!d.poll(5.0));
}

#[test]
fn the_delay_is_reported() {
    let d = Debounce::new(0.25);
    assert!((d.delay() - 0.25).abs() < 1e-9);
}

#[test]
fn a_negative_delay_is_clamped_rather_than_inverting_the_logic() {
    let mut d = Debounce::new(-1.0);
    assert!(d.delay() >= 0.0);
    d.note_edit(1.0);
    assert!(d.poll(1.0));
}
