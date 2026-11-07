//! Interaction state that has rules worth testing without a window.
//!
//! These are the pieces of "how the interface behaves over time": messages that
//! must expire, destructive actions that need a second click, and a mute toggle
//! that has to remember what the volume was.

/// A short-lived message shown in the status bar.
///
/// Permanent status text is a UX smell: after five minutes the user cannot tell
/// whether "已保存" refers to what they just did or to something from lunch.
#[derive(Debug, Clone)]
pub struct Toast {
    message: String,
    error: bool,
    shown_at: Option<f64>,
}

impl Toast {
    /// How long a message stays on screen.
    pub const LIFETIME: f64 = 4.0;

    /// Create an empty toast.
    pub fn new() -> Self {
        Self {
            message: String::new(),
            error: false,
            shown_at: None,
        }
    }

    /// Show a message, replacing whatever was there.
    pub fn show(&mut self, message: impl Into<String>, error: bool, now: f64) {
        self.message = message.into();
        self.error = error;
        self.shown_at = Some(now);
    }

    /// The message to display right now, if it has not expired.
    pub fn current(&self, now: f64) -> Option<(&str, bool)> {
        let at = self.shown_at?;
        if now - at >= Self::LIFETIME {
            None
        } else {
            Some((&self.message, self.error))
        }
    }

    /// True while a repaint is needed to expire the message.
    pub fn is_active(&self) -> bool {
        self.shown_at.is_some()
    }

    /// Drop the message immediately.
    pub fn clear(&mut self) {
        self.shown_at = None;
        self.message.clear();
    }
}

impl Default for Toast {
    fn default() -> Self {
        Self::new()
    }
}

/// A two-step confirmation for an irreversible action.
///
/// Clearing a cache can destroy gigabytes of downloads, so one click must not
/// do it. Two clicks with a visible time limit is the lightest safe pattern —
/// lighter than a modal, harder to trigger by accident than a single button.
#[derive(Debug, Clone)]
pub struct Confirm {
    armed_at: Option<f64>,
}

impl Confirm {
    /// How long the first click stays armed.
    pub const WINDOW: f64 = 3.0;

    /// Create a disarmed confirmation.
    pub fn new() -> Self {
        Self { armed_at: None }
    }

    /// Register a click. Returns true when the action may proceed.
    pub fn click(&mut self, now: f64) -> bool {
        match self.armed_at {
            Some(at) if now - at < Self::WINDOW => {
                self.armed_at = None;
                true
            }
            _ => {
                self.armed_at = Some(now);
                false
            }
        }
    }

    /// True while the first click is pending, so the label can change.
    pub fn is_armed(&self, now: f64) -> bool {
        self.armed_at
            .map(|at| now - at < Self::WINDOW)
            .unwrap_or(false)
    }

    /// Disarm, e.g. because the pointer left the control.
    pub fn cancel(&mut self) {
        self.armed_at = None;
    }
}

impl Default for Confirm {
    fn default() -> Self {
        Self::new()
    }
}

/// Volume with a mute toggle that remembers the previous level.
#[derive(Debug, Clone)]
pub struct Volume {
    level: f32,
    muted: bool,
}

impl Volume {
    /// Level used when unmuting from silence.
    pub const RESTORE: f32 = 0.5;

    /// Start at full volume.
    pub fn new() -> Self {
        Self {
            level: 1.0,
            muted: false,
        }
    }

    /// The level to send to the engine; zero when muted.
    pub fn effective(&self) -> f32 {
        if self.muted {
            0.0
        } else {
            self.level
        }
    }

    /// The level the slider should show (unchanged while muted).
    pub fn visible(&self) -> f32 {
        self.level
    }

    /// True when muted.
    pub fn is_muted(&self) -> bool {
        self.muted
    }

    /// Move the slider. Any non-zero level unmutes.
    pub fn set(&mut self, level: f32) {
        let level = level.clamp(0.0, 1.0);
        self.level = level;
        self.muted = level <= f32::EPSILON;
    }

    /// Toggle mute. Unmuting from zero restores a sensible level rather than
    /// silently leaving the user at silence.
    pub fn toggle_mute(&mut self) {
        if self.muted {
            self.muted = false;
            if self.level <= f32::EPSILON {
                self.level = Self::RESTORE;
            }
        } else {
            self.muted = true;
        }
    }
}

impl Default for Volume {
    fn default() -> Self {
        Self::new()
    }
}
