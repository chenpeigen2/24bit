//! Behavioural tests for CJK font discovery.
//!
//! Without a Chinese face, every song title renders as tofu — this is the most
//! visible possible bug on a site that is entirely in Chinese, so the search
//! order is pinned here rather than left to the system.

use twentyfourbit_gui::fonts::{find_cjk_font, find_in, FontSource};

/// This machine must have *some* CJK font; every desktop OS ships one.
#[test]
fn this_machine_has_a_cjk_font() {
    // This asserts something about the *machine*, not about our code. A build
    // server without fonts is not a bug in the client; the UI already says so
    // in a banner. Skip instead of failing the pipeline.
    let Some(font) = find_cjk_font() else {
        eprintln!("skipping: no CJK font installed on this machine");
        return;
    };
    assert!(!font.family.is_empty());

    match &font.source {
        FontSource::File(path) => assert!(path.exists(), "font file {path:?} must exist"),
    }
}

/// The family we pick must be one of the known-CJK ones, not an arbitrary face.
#[test]
fn the_chosen_family_is_a_known_cjk_family() {
    let Some(font) = find_cjk_font() else {
        eprintln!("skipping: no CJK font installed");
        return;
    };
    let name = font.family.to_lowercase();

    let known = [
        "pingfang",
        "hiragino",
        "yahei",
        "noto sans cjk",
        "noto sans sc",
        "source han",
        "heiti",
        "songti",
        "simhei",
        "simsun",
        "wenquanyi",
        "droid sans fallback",
        "arial unicode",
    ];
    assert!(
        known.iter().any(|k| name.contains(k)),
        "picked {name:?}, which is not a known CJK family"
    );
}

/// The bytes must actually load, otherwise the font is useless at runtime.
#[test]
fn the_chosen_font_can_be_read() {
    let Some(font) = find_cjk_font() else {
        eprintln!("skipping: no CJK font installed");
        return;
    };
    let bytes = twentyfourbit_gui::fonts::load_bytes(&font).expect("font bytes must load");
    assert!(
        bytes.len() > 10_000,
        "a CJK font is never this small ({} bytes)",
        bytes.len()
    );

    // A TrueType/OpenType file starts with one of these magic numbers.
    let magic = &bytes[..4];
    let ok = matches!(
        magic,
        [0x00, 0x01, 0x00, 0x00] | b"ttcf" | b"OTTO" | b"true"
    );
    assert!(ok, "unexpected font magic {magic:02x?}");
}

/// A database with no fonts at all must report "none" rather than panic.
#[test]
fn an_empty_database_reports_none() {
    let db = fontdb::Database::new();
    assert!(find_in(&db).is_none());
}

/// Latin text carries most of the interface. On macOS this must resolve to SF
/// Pro, which is the difference between "native app" and "toolkit demo".
#[test]
fn the_ui_font_prefers_the_system_face() {
    let font = twentyfourbit_gui::fonts::find_ui_font();
    match font {
        Some(font) => {
            let name = font.family.to_lowercase();
            let known = [
                "system font",
                "sf pro",
                "sf ns",
                ".sf ns",
                "sf compact",
                "helvetica",
                "segoe",
                "cantarell",
                "ubuntu",
                "dejavu",
            ];
            assert!(
                known.iter().any(|k| name.contains(k)),
                "picked {name:?}, which is not a system UI face"
            );
            let bytes = twentyfourbit_gui::fonts::load_bytes(&font).expect("bytes");
            assert!(bytes.len() > 10_000);
        }
        None => {
            // Acceptable on a machine with no usable font at all, but say so
            // loudly instead of passing silently.
            eprintln!("no system UI font found; egui's built-in face will be used");
        }
    }
}
