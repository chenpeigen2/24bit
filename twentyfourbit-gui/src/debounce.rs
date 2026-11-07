//! Debounce for the search box.
//!
//! Typing "富士山下" is four keystrokes; searching on each one would fire four
//! requests and flicker the result list. The debounce waits until typing settles.
//!
//! Time is passed in rather than read from a clock, so the behaviour is exactly
//! testable.

/// Fires once, `delay` seconds after the last edit.
#[derive(Debug, Clone)]
pub struct Debounce {
    delay: f64,
    /// Timestamp of the most recent edit, if one is waiting.
    pending: Option<f64>,
}

impl Debounce {
    /// Create a debounce with the given delay in seconds.
    pub fn new(delay: f64) -> Self {
        Self {
            delay: delay.max(0.0),
            pending: None,
        }
    }

    /// The configured delay.
    pub fn delay(&self) -> f64 {
        self.delay
    }

    /// Record that the user edited the text at `now`.
    pub fn note_edit(&mut self, now: f64) {
        self.pending = Some(now);
    }

    /// True exactly once, when the delay since the last edit has elapsed.
    pub fn poll(&mut self, now: f64) -> bool {
        match self.pending {
            Some(at) if now - at >= self.delay => {
                self.pending = None;
                true
            }
            _ => false,
        }
    }

    /// Drop any pending firing, e.g. because the user pressed Enter and searched
    /// immediately.
    pub fn cancel(&mut self) {
        self.pending = None;
    }

    /// True while an edit is waiting to fire.
    pub fn is_pending(&self) -> bool {
        self.pending.is_some()
    }
}
