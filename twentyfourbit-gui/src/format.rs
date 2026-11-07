//! Display formatting.

use std::time::Duration;

/// Render a duration the way a music player does.
pub fn format_duration(d: Duration) -> String {
    let total = d.as_secs();
    let (h, m, s) = (total / 3600, (total % 3600) / 60, total % 60);
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    }
}

/// Render a byte count in binary units.
pub fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 6] = ["B", "KB", "MB", "GB", "TB", "PB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

/// Render a progress fraction as a percentage.
///
/// Returns an empty string for a non-finite value: a download with no
/// content-length has no progress to show, and inventing "NaN%" would be worse
/// than showing nothing.
pub fn format_progress(fraction: f64) -> String {
    if !fraction.is_finite() {
        return String::new();
    }
    format!("{}%", (fraction.clamp(0.0, 1.0) * 100.0).round() as u32)
}
