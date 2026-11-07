//! Device-independent playback planning.
//!
//! The 24bit master tier is 24bit/192kHz. Most consumer output devices cannot
//! accept that rate, so the player must decide *before* touching audio hardware
//! what rate to ask for and whether to resample. Keeping that decision pure is
//! what makes it testable on machines with no sound card.

/// How a source rate relates to the rate an output device will accept.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResampleMode {
    /// The device accepts the file's native rate.
    Passthrough,
    /// The device runs slower than the file, so frames must be discarded.
    Downsample,
    /// The device runs faster than the file, so frames must be interpolated.
    Upsample,
}

/// The rate used when a device refuses to report its capabilities.
///
/// 48kHz is supported by effectively every output device shipped this century,
/// which makes it the safest universal fallback.
pub const SAFE_DEFAULT_RATE: u32 = 48_000;

/// A resolved decision about how to feed one file to one device.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlaybackPlan {
    /// Sample rate of the decoded file.
    pub source_rate: u32,
    /// Sample rate that will be requested from the device.
    pub output_rate: u32,
    /// What has to happen to the audio.
    pub mode: ResampleMode,
}

impl PlaybackPlan {
    /// Decide how to play `source_rate` audio on a device limited to
    /// `device_rate`.
    ///
    /// A `device_rate` of zero means "the device did not tell us", which is
    /// treated as [`SAFE_DEFAULT_RATE`] rather than as an error: refusing to
    /// play because a driver was coy would be worse than a safe downsample.
    pub fn for_source(source_rate: u32, device_rate: u32) -> Self {
        let output_rate = if device_rate == 0 {
            SAFE_DEFAULT_RATE
        } else {
            device_rate
        };

        let mode = if source_rate == 0 || source_rate == output_rate {
            ResampleMode::Passthrough
        } else if source_rate > output_rate {
            ResampleMode::Downsample
        } else {
            ResampleMode::Upsample
        };

        Self {
            source_rate,
            output_rate,
            mode,
        }
    }

    /// Output frames per source frame. Always 1.0 for passthrough, and 1.0 for
    /// a zero source rate so that callers never divide by zero.
    pub fn ratio(&self) -> f64 {
        if self.source_rate == 0 {
            1.0
        } else {
            self.output_rate as f64 / self.source_rate as f64
        }
    }

    /// How many frames the output will actually carry for a given input length.
    ///
    /// Rounds to nearest: truncating would silently drop up to a frame per
    /// second, which accumulates into audible drift on long tracks.
    pub fn effective_frames(&self, source_frames: u64) -> u64 {
        (source_frames as f64 * self.ratio()).round() as u64
    }

    /// True when the audio must pass through a resampler.
    pub fn needs_resampling(&self) -> bool {
        !matches!(self.mode, ResampleMode::Passthrough)
    }
}
