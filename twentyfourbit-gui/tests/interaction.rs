//! Behavioural tests for the interaction rules that live outside the window.
//!
//! These cover the parts users notice when they are wrong: a message that never
//! goes away, a destructive button that fires on the first click, a mute button
//! that loses the volume.

use twentyfourbit_gui::ui_state::{Confirm, Toast, Volume};

// ------------------------------------------------------------------- toast

#[test]
fn a_toast_disappears_on_its_own() {
    let mut toast = Toast::new();
    toast.show("已保存", false, 10.0);

    assert_eq!(toast.current(10.0), Some(("已保存", false)));
    assert_eq!(toast.current(13.9), Some(("已保存", false)), "still visible");
    assert_eq!(toast.current(14.0), None, "expired after its lifetime");
}

#[test]
fn a_new_toast_replaces_the_old_one_and_restarts_the_clock() {
    let mut toast = Toast::new();
    toast.show("第一条", false, 0.0);
    toast.show("第二条", true, 3.0);

    assert_eq!(toast.current(3.5), Some(("第二条", true)));
    assert_eq!(
        toast.current(6.9),
        Some(("第二条", true)),
        "the timer restarted when the message changed"
    );
    assert_eq!(toast.current(7.1), None);
}

#[test]
fn an_empty_toast_shows_nothing() {
    let toast = Toast::new();
    assert_eq!(toast.current(0.0), None);
    assert!(!toast.is_active());
}

#[test]
fn an_error_toast_is_marked_as_an_error() {
    let mut toast = Toast::new();
    toast.show("网络错误", true, 0.0);
    let (message, error) = toast.current(0.0).unwrap();
    assert_eq!(message, "网络错误");
    assert!(error);
}

// ----------------------------------------------------------------- confirm

#[test]
fn a_destructive_action_needs_two_clicks() {
    let mut confirm = Confirm::new();

    assert!(!confirm.click(0.0), "the first click only arms");
    assert!(confirm.is_armed(0.0));
    assert!(confirm.click(0.5), "the second click proceeds");
    assert!(!confirm.is_armed(0.5), "and disarms afterwards");
}

#[test]
fn the_confirmation_expires_so_it_cannot_be_triggered_much_later() {
    let mut confirm = Confirm::new();

    assert!(!confirm.click(0.0));
    assert!(!confirm.is_armed(3.5), "an old first click no longer counts");
    assert!(!confirm.click(3.5), "so this arms again instead of firing");
    assert!(confirm.click(3.6));
}

#[test]
fn leaving_the_control_cancels_the_confirmation() {
    let mut confirm = Confirm::new();
    assert!(!confirm.click(0.0));
    confirm.cancel();
    assert!(!confirm.is_armed(0.1));
    assert!(!confirm.click(0.2), "a fresh click must arm again");
}

// ------------------------------------------------------------------ volume

#[test]
fn muting_keeps_the_level_the_user_had() {
    let mut volume = Volume::new();
    volume.set(0.7);
    assert_eq!(volume.effective(), 0.7);

    volume.toggle_mute();
    assert_eq!(volume.effective(), 0.0, "muted sends silence");
    assert_eq!(volume.visible(), 0.7, "but the slider keeps its position");

    volume.toggle_mute();
    assert_eq!(volume.effective(), 0.7, "unmuting restores the level");
}

#[test]
fn unmuting_from_silence_restores_a_sensible_level() {
    let mut volume = Volume::new();
    volume.set(0.0);
    assert!(volume.is_muted(), "dragging to zero is a mute");

    volume.toggle_mute();
    assert!(!volume.is_muted());
    assert!(
        volume.effective() > 0.0,
        "unmuting from zero must not leave the user in silence"
    );
    assert_eq!(volume.effective(), Volume::RESTORE);
}

#[test]
fn moving_the_slider_unmutes() {
    let mut volume = Volume::new();
    volume.toggle_mute();
    assert!(volume.is_muted());

    volume.set(0.3);
    assert!(!volume.is_muted(), "touching the slider implies wanting sound");
    assert_eq!(volume.effective(), 0.3);
}

#[test]
fn the_level_is_clamped() {
    let mut volume = Volume::new();
    volume.set(5.0);
    assert_eq!(volume.visible(), 1.0);
    volume.set(-2.0);
    assert_eq!(volume.visible(), 0.0);
}

// ------------------------------------------------------------------- glass

/// Surfaces must stay opaque when there is no blurred backdrop behind them.
#[test]
fn surfaces_are_opaque_without_a_backdrop() {
    twentyfourbit_gui::theme::set_glass(false);
    for colour in [
        twentyfourbit_gui::theme::bg(),
        twentyfourbit_gui::theme::panel(),
        twentyfourbit_gui::theme::card(),
        twentyfourbit_gui::theme::card_hover(),
    ] {
        assert_eq!(colour.a(), 255, "an opaque window must not be see-through");
    }
}

/// With vibrancy on, chrome is more translucent than the content plane, which
/// stays readable.
#[test]
fn glass_makes_chrome_translucent_but_keeps_content_readable() {
    twentyfourbit_gui::theme::set_glass(true);

    let panel = twentyfourbit_gui::theme::panel().a();
    let content = twentyfourbit_gui::theme::bg().a();

    assert!(panel < 160, "chrome should read as glass, got alpha {panel}");
    assert!(
        content >= 190,
        "body text sits on the content plane and needs a real scrim, got alpha {content}"
    );
    assert!(
        panel < content,
        "chrome must be more translucent than content for the layering to read"
    );

    twentyfourbit_gui::theme::set_glass(false);
}
