//! Streaming sample-rate conversion with a windowed-sinc kernel.
//!
//! 24bit.net's master tier is 24bit/192kHz; almost no output device accepts
//! that, so everything is downsampled on the way out. A box filter (what this
//! crate used to do) folds everything above the *output* Nyquist back into the
//! audible band, so a 30kHz component lands as an 18kHz whistle. A proper
//! anti-aliasing kernel removes it, which is what `rubato` gives us.

use rubato::audioadapter_buffers::direct::InterleavedSlice;
use rubato::{
    Async, FixedAsync, Indexing, Resampler, SincInterpolationParameters, SincInterpolationType,
    WindowFunction,
};

/// Frames processed per resampler call. Larger is cheaper and smoother; this
/// value keeps latency around a few milliseconds at typical rates.
const CHUNK_FRAMES: usize = 1024;

/// A streaming interleaved resampler.
pub struct StreamResampler {
    inner: Option<Async<f32>>,
    channels: usize,
    source_rate: u32,
    target_rate: u32,
    /// Input staged until a full chunk is available.
    pending: Vec<f32>,
    /// Scratch buffer for one chunk of output.
    out_scratch: Vec<f32>,
}

impl StreamResampler {
    /// Create a resampler from `source_rate` to `target_rate`.
    ///
    /// Returns `Ok` with `inner == None` for a passthrough when the rates match,
    /// so callers do not need a special case.
    pub fn new(source_rate: u32, target_rate: u32, channels: usize) -> Result<Self, String> {
        let channels = channels.max(1);
        if source_rate == 0 || target_rate == 0 {
            return Err(format!(
                "invalid sample rates: source {source_rate}, target {target_rate}"
            ));
        }

        if source_rate == target_rate {
            return Ok(Self {
                inner: None,
                channels,
                source_rate,
                target_rate,
                pending: Vec::new(),
                out_scratch: Vec::new(),
            });
        }

        let ratio = target_rate as f64 / source_rate as f64;
        // 256 taps with a Blackman-Harris window is a good accuracy/cost point:
        // stopband attenuation well below the threshold this crate's tests
        // measure, at a fraction of the cost of a 512-tap kernel.
        let params = SincInterpolationParameters {
            sinc_len: 256,
            f_cutoff: None,
            interpolation: SincInterpolationType::Linear,
            oversampling_factor: 256,
            window: WindowFunction::BlackmanHarris2,
        };

        let inner = Async::<f32>::new_sinc(
            ratio,
            1.1,
            &params,
            CHUNK_FRAMES,
            channels,
            FixedAsync::Input,
        )
        .map_err(|e| format!("cannot build resampler: {e}"))?;

        let out_scratch = vec![0.0f32; channels * inner.output_frames_max()];

        Ok(Self {
            inner: Some(inner),
            channels,
            source_rate,
            target_rate,
            pending: Vec::new(),
            out_scratch,
        })
    }

    /// The rate samples come out at.
    pub fn target_rate(&self) -> u32 {
        self.target_rate
    }

    /// The rate samples go in at.
    pub fn source_rate(&self) -> u32 {
        self.source_rate
    }

    /// True when this instance is a no-op.
    pub fn is_passthrough(&self) -> bool {
        self.inner.is_none()
    }

    /// Feed interleaved `input` samples, appending converted samples to `out`.
    ///
    /// Partial chunks are staged internally so callers can push arbitrary sizes,
    /// including awkward ones from a decoder loop.
    pub fn push(&mut self, input: &[f32], out: &mut Vec<f32>) {
        let Some(inner) = self.inner.as_mut() else {
            out.extend_from_slice(input);
            return;
        };

        self.pending.extend_from_slice(input);
        let chunk_samples = CHUNK_FRAMES * self.channels;

        while self.pending.len() >= chunk_samples {
            let chunk: Vec<f32> = self.pending.drain(..chunk_samples).collect();
            let channels = self.channels;
            process_chunk(inner, &mut self.out_scratch, channels, &chunk, CHUNK_FRAMES, out);
        }
    }

    /// Flush the tail: the staged partial chunk is processed as a final,
    /// shorter block and the resampler's internal delay is drained.
    pub fn flush(&mut self, out: &mut Vec<f32>) {
        let Some(inner) = self.inner.as_mut() else {
            return;
        };

        let leftover_frames = self.pending.len() / self.channels;
        if leftover_frames > 0 {
            let mut tail = std::mem::take(&mut self.pending);
            // Pad to a full chunk so the adapter geometry stays valid; the
            // partial length tells the resampler to ignore the padding.
            tail.resize(CHUNK_FRAMES * self.channels, 0.0);
            let channels = self.channels;
            process_chunk(inner, &mut self.out_scratch, channels, &tail, leftover_frames, out);
        }
    }
}

fn process_chunk(
        inner: &mut Async<f32>,
        out_scratch: &mut [f32],
        channels: usize,
        chunk: &[f32],
        valid_frames: usize,
        out: &mut Vec<f32>,
    ) {
        let indexing = Indexing {
            partial_len: (valid_frames < CHUNK_FRAMES).then_some(valid_frames),
            ..Indexing::default()
        };

        let input_adapter = match InterleavedSlice::new(chunk, channels, CHUNK_FRAMES) {
            Ok(a) => a,
            Err(e) => {
                tracing::error!("resampler input adapter rejected the chunk: {e}");
                return;
            }
        };
        let capacity = out_scratch.len() / channels;
        let mut output_adapter =
            match InterleavedSlice::new_mut(out_scratch, channels, capacity) {
                Ok(a) => a,
                Err(e) => {
                    tracing::error!("resampler output adapter rejected the buffer: {e}");
                    return;
                }
            };

        match inner.process_into_buffer(&input_adapter, &mut output_adapter, Some(&indexing)) {
            Ok((_read, written)) => {
                out.extend_from_slice(&out_scratch[..written * channels]);
            }
            Err(e) => tracing::error!("resampling failed: {e}"),
        }
    }
