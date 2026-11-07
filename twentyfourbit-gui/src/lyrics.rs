//! LRC lyrics parsing.
//!
//! The master quality tier ships lyrics with every track, so they arrive for
//! free — this turns the raw LRC text into something a UI can scroll and
//! highlight.
//!
//! LRC is `[mm:ss.xxx]text`, one line at a time, but the format in the wild is
//! looser than that:
//!
//! * one line may carry several timestamps (a repeated chorus)
//! * the fraction may be absent, or one, two, or three digits
//! * `[offset:...]` shifts every timestamp in the file
//! * hand-edited files are frequently out of order
//!
//! All of those are handled here rather than at the call site.

use std::time::Duration;

/// One sung line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LyricLine {
    /// When the line starts.
    pub at: Duration,
    /// What is sung.
    pub text: String,
}

/// A whole track's lyrics, sorted and offset-adjusted.
#[derive(Debug, Clone, Default)]
pub struct Lyrics {
    /// Lines in chronological order.
    pub lines: Vec<LyricLine>,
}

impl Lyrics {
    /// Parse an LRC document.
    ///
    /// Never fails: malformed lines are skipped, because a partially readable
    /// lyric sheet is more useful than none.
    pub fn parse(text: &str) -> Self {
        let mut lines = Vec::new();
        let mut offset_ms: i64 = 0;

        for raw in text.lines() {
            let mut rest = raw.trim();

            if rest.is_empty() {
                continue;
            }

            // Consume every leading `[...]` group. Tags that are not timestamps
            // carry metadata (`ti`, `ar`, `offset`, …) and are handled here.
            let mut stamps: Vec<i64> = Vec::new();
            while let Some(close) = rest.find(']') {
                // A timestamp group must start with '['.
                if !rest.starts_with('[') {
                    break;
                }
                let inside = &rest[1..close];

                if let Some(ms) = parse_tag(inside, "offset") {
                    offset_ms = ms;
                } else if let Some(ms) = parse_stamp(inside) {
                    stamps.push(ms);
                }
                // Anything else (ti/ar/al/by/…) is metadata: ignore.

                rest = rest[close + 1..].trim_start();
                if !rest.starts_with('[') {
                    break;
                }
            }

            if stamps.is_empty() {
                // Either a metadata-only line or unrecognised junk.
                continue;
            }

            let body = rest.trim();
            if body.is_empty() {
                continue;
            }

            for ms in stamps {
                lines.push((ms, body.to_owned()));
            }
        }

        let mut lines: Vec<LyricLine> = lines
            .into_iter()
            .map(|(ms, text)| LyricLine {
                at: Duration::from_millis(shift(ms, offset_ms)),
                text,
            })
            .collect();

        lines.sort_by_key(|l| l.at);
        Self { lines }
    }

    /// True when there is nothing to show.
    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }

    /// Index of the line being sung at `position`, if any.
    ///
    /// Returns `None` before the first line; past the end it returns the last
    /// line, because the final line stays on screen until the track ends.
    pub fn index_at(&self, position: Duration) -> Option<usize> {
        if self.lines.is_empty() || position < self.lines[0].at {
            return None;
        }
        // The list is sorted, so find the last line starting at or before
        // `position`.
        let idx = self.lines.partition_point(|l| l.at <= position);
        Some(idx.saturating_sub(1).min(self.lines.len() - 1))
    }
}

/// `[mm:ss]` or `[mm:ss.fff]`, also accepting hours as `[hh:mm:ss.fff]`.
fn parse_stamp(inside: &str) -> Option<i64> {
    let (time, frac) = match inside.split_once('.') {
        Some((t, f)) => (t, Some(f)),
        None => (inside, None),
    };

    let parts: Vec<&str> = time.split(':').collect();
    let (hours, minutes, seconds) = match parts.as_slice() {
        [m, s] => (0u64, m.parse::<u64>().ok()?, s.parse::<u64>().ok()?),
        [h, m, s] => (
            h.parse::<u64>().ok()?,
            m.parse::<u64>().ok()?,
            s.parse::<u64>().ok()?,
        ),
        _ => return None,
    };

    let frac_ms = match frac {
        None => 0,
        Some(f) => {
            if f.is_empty() || !f.chars().all(|c| c.is_ascii_digit()) {
                return None;
            }
            // `.5` is half a second, `.50` is half a second, `.500` likewise.
            let value: u64 = f.parse().ok()?;
            let scale = 10u64.pow(f.len() as u32);
            (value * 1000) / scale
        }
    };

    Some(((hours * 3600 + minutes * 60 + seconds) * 1000 + frac_ms) as i64)
}

/// `offset:+500` / `offset:-500`, in milliseconds.
fn parse_tag(inside: &str, name: &str) -> Option<i64> {
    let (key, value) = inside.split_once(':')?;
    if !key.trim().eq_ignore_ascii_case(name) {
        return None;
    }
    value.trim().parse::<i64>().ok()
}

/// Apply an offset without underflowing below zero.
fn shift(ms: i64, offset: i64) -> u64 {
    (ms + offset).max(0) as u64
}
