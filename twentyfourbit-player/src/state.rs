//! The transport state machine.
//!
//! Pure — no audio hardware, no threads. The engine owns one of these behind a
//! mutex and the GUI reads it every frame, so every transition must be total
//! and every value clamped.

use std::time::Duration;

/// What the transport is currently doing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    /// Nothing loaded, or stopped.
    Idle,
    /// Advancing.
    Playing,
    /// Loaded and positioned, but not advancing.
    Paused,
    /// Reached the end of the media.
    Finished,
}

/// Position, phase, and volume for one loaded track.
#[derive(Debug, Clone)]
pub struct Transport {
    phase: Phase,
    position_frames: u64,
    total_frames: u64,
    sample_rate: u32,
    #[allow(dead_code)]
    channels: u16,
    volume: f32,
}

impl Transport {
    /// Create an idle transport for media of the given length.
    pub fn new(total: Duration, sample_rate: u32, channels: u16) -> Self {
        Self {
            phase: Phase::Idle,
            position_frames: 0,
            total_frames: frames_of(total, sample_rate),
            sample_rate,
            channels,
            volume: 1.0,
        }
    }

    /// Current phase.
    pub fn phase(&self) -> Phase {
        self.phase
    }

    /// Current playback position.
    pub fn position(&self) -> Duration {
        Duration::from_secs_f64(self.position_frames as f64 / self.sample_rate as f64)
    }

    /// Length of the loaded media.
    pub fn duration(&self) -> Duration {
        Duration::from_secs_f64(self.total_frames as f64 / self.sample_rate as f64)
    }

    /// Position as a fraction in `0.0..=1.0`. Zero-length media reports 0.
    pub fn progress(&self) -> f64 {
        if self.total_frames == 0 {
            0.0
        } else {
            (self.position_frames as f64 / self.total_frames as f64).clamp(0.0, 1.0)
        }
    }

    /// Playback volume in `0.0..=1.0`.
    pub fn volume(&self) -> f32 {
        self.volume
    }

    /// True while audio should be advancing.
    pub fn is_playing(&self) -> bool {
        self.phase == Phase::Playing
    }

    /// Start (or restart) playback from the current position.
    pub fn play(&mut self) {
        if self.phase == Phase::Finished {
            self.position_frames = 0;
        }
        self.phase = Phase::Playing;
    }

    /// Pause, keeping the position.
    pub fn pause(&mut self) {
        if self.phase == Phase::Playing {
            self.phase = Phase::Paused;
        }
    }

    /// Resume from the current position.
    pub fn resume(&mut self) {
        if self.phase == Phase::Paused {
            self.phase = Phase::Playing;
        }
    }

    /// Stop and return to the beginning.
    pub fn stop(&mut self) {
        self.phase = Phase::Idle;
        self.position_frames = 0;
    }

    /// Jump to an absolute position, clamped to the media bounds.
    pub fn seek(&mut self, to: Duration) {
        let frames = frames_of(to, self.sample_rate);
        self.position_frames = frames.min(self.total_frames);
    }

    /// Jump to a fraction of the media, clamped to `0.0..=1.0`.
    pub fn seek_fraction(&mut self, fraction: f64) {
        let f = fraction.clamp(0.0, 1.0);
        self.position_frames = (self.total_frames as f64 * f).round() as u64;
    }

    /// Set the volume, clamped to `0.0..=1.0`.
    pub fn set_volume(&mut self, v: f32) {
        self.volume = v.clamp(0.0, 1.0);
    }

    /// Advance the position by `frames` of device output.
    ///
    /// Ignored unless playing. Reaching the end moves to [`Phase::Finished`]
    /// and clamps the position, so a UI can always trust `progress() <= 1.0`.
    pub fn advance_frames(&mut self, frames: u64) {
        if self.phase != Phase::Playing {
            return;
        }
        self.position_frames = self.position_frames.saturating_add(frames);
        if self.position_frames >= self.total_frames {
            self.position_frames = self.total_frames;
            self.phase = Phase::Finished;
        }
    }

    /// Mark playback complete, pinning the position at the end.
    pub(crate) fn finish(&mut self) {
        self.position_frames = self.total_frames;
        self.phase = Phase::Finished;
    }

}

fn frames_of(d: Duration, sample_rate: u32) -> u64 {
    (d.as_secs_f64() * sample_rate as f64).round() as u64
}
