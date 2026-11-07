//! The actual audio pipeline: decode → resample → output.
//!
//! 24bit.net serves 24bit/192kHz FLAC. Almost no consumer output device accepts
//! that rate, so the pipeline always consults [`crate::plan`] before opening a
//! stream and resamples when the device cannot take the file natively.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use crate::plan::{PlaybackPlan, ResampleMode};

/// Playback failure.
#[derive(Debug, thiserror::Error)]
pub enum PlayError {
    /// The file could not be opened.
    #[error("cannot open {path}: {source}")]
    Open {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    /// No output device exists at all.
    #[error("no audio output device is available")]
    NoDevice,

    /// The output device refused the stream configuration.
    #[error("cannot open the output stream: {0}")]
    Device(String),

    /// The container or codec could not be decoded.
    #[error("cannot decode {path}: {reason}")]
    Decode { path: PathBuf, reason: String },

    /// The file contains no recognisable audio track.
    #[error("no audio track in {path}")]
    UnsupportedFormat { path: PathBuf },
}

/// Play a local audio file to completion on the default output device.
///
/// Returns once playback finishes. The file is fully decoded before output
/// begins, which keeps the pipeline simple and avoids dropouts on the huge
/// master-tier files.
pub fn play_file(path: &Path) -> Result<(), PlayError> {
    let track = decode(path)?;
    let plan = PlaybackPlan::for_source(track.sample_rate, device_max_rate());

    tracing::debug!(
        source_rate = plan.source_rate,
        output_rate = plan.output_rate,
        mode = ?plan.mode,
        "playback plan resolved"
    );

    let samples = if plan.needs_resampling() {
        resample(&track.samples, &plan)
    } else {
        track.samples
    };

    output(&samples, plan.output_rate)
}

/// A fully decoded file, before resampling.
struct DecodedTrack {
    /// Interleaved f32 samples.
    samples: Vec<f32>,
    sample_rate: u32,
}

fn decode(path: &Path) -> Result<DecodedTrack, PlayError> {
    use symphonia::core::codecs::audio::AudioDecoderOptions;
    use symphonia::core::formats::probe::Hint;
    use symphonia::core::formats::{FormatOptions, TrackType};
    use symphonia::core::io::MediaSourceStream;
    use symphonia::core::meta::MetadataOptions;

    let file = std::fs::File::open(path).map_err(|source| PlayError::Open {
        path: path.to_path_buf(),
        source,
    })?;

    let mss = MediaSourceStream::new(Box::new(file), Default::default());

    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }

    let mut format = symphonia::default::get_probe()
        .probe(
            &hint,
            mss,
            FormatOptions::default(),
            MetadataOptions::default(),
        )
        .map_err(|e| PlayError::Decode {
            path: path.to_path_buf(),
            reason: e.to_string(),
        })?;

    let track = format
        .default_track(TrackType::Audio)
        .ok_or_else(|| PlayError::UnsupportedFormat {
            path: path.to_path_buf(),
        })?;

    let track_id = track.id;
    let audio_params = track
        .codec_params
        .as_ref()
        .and_then(|p| p.audio())
        .ok_or_else(|| PlayError::UnsupportedFormat {
            path: path.to_path_buf(),
        })?
        .clone();

    let mut decoder = symphonia::default::get_codecs()
        .make_audio_decoder(&audio_params, &AudioDecoderOptions::default())
        .map_err(|e| PlayError::Decode {
            path: path.to_path_buf(),
            reason: e.to_string(),
        })?;

    let mut samples: Vec<f32> = Vec::new();
    let mut sample_rate = audio_params.sample_rate.unwrap_or(0);

    loop {
        let packet = match format.next_packet() {
            Ok(Some(p)) => p,
            Ok(None) => break,
            Err(symphonia::core::errors::Error::IoError(e))
                if e.kind() == std::io::ErrorKind::UnexpectedEof =>
            {
                break
            }
            Err(symphonia::core::errors::Error::ResetRequired) => break,
            Err(e) => {
                return Err(PlayError::Decode {
                    path: path.to_path_buf(),
                    reason: e.to_string(),
                })
            }
        };

        if packet.track_id != track_id {
            continue;
        }

        let decoded = match decoder.decode(&packet) {
            Ok(d) => d,
            Err(symphonia::core::errors::Error::DecodeError(_)) => continue,
            Err(e) => {
                return Err(PlayError::Decode {
                    path: path.to_path_buf(),
                    reason: e.to_string(),
                })
            }
        };

        if sample_rate == 0 {
            sample_rate = decoded.spec().rate();
        }

        // `copy_to_vec_interleaved` overwrites the destination rather than
        // appending, so each packet lands in a scratch buffer first. (Verified
        // against symphonia 0.6: accumulating directly would silently keep only
        // the last packet's samples — a 5-second file would play as 15ms.)
        let mut scratch: Vec<f32> = Vec::new();
        decoded.copy_to_vec_interleaved(&mut scratch);
        samples.extend_from_slice(&scratch);
    }

    if samples.is_empty() {
        return Err(PlayError::Decode {
            path: path.to_path_buf(),
            reason: "file contains no audio frames".to_owned(),
        });
    }

    Ok(DecodedTrack {
        samples,
        sample_rate,
    })
}

/// The highest sample rate the default output device will accept, or 0 when the
/// device cannot be interrogated (which the plan turns into a safe default).
fn device_max_rate() -> u32 {
    use cpal::traits::{DeviceTrait, HostTrait};

    let host = cpal::default_host();
    let Some(device) = host.default_output_device() else {
        return 0;
    };
    let Ok(configs) = device.supported_output_configs() else {
        return 0;
    };
    configs.map(|c| c.max_sample_rate()).max().unwrap_or(0)
}

/// Resample interleaved f32 samples according to `plan`.
///
/// A box filter is used for downsampling and linear interpolation for
/// upsampling. Both are deliberately simple: the master tier carries enormous
/// oversampling headroom, so the difference from a windowed-sinc kernel is
/// inaudible, and this keeps the crate free of a heavy DSP dependency.
fn resample(samples: &[f32], plan: &PlaybackPlan) -> Vec<f32> {
    if !plan.needs_resampling() || samples.is_empty() {
        return samples.to_vec();
    }

    let ratio = plan.ratio();
    match plan.mode {
        ResampleMode::Passthrough => samples.to_vec(),
        ResampleMode::Downsample => downsample(samples, ratio),
        ResampleMode::Upsample => {
            let out_len = plan.effective_frames(samples.len() as u64) as usize;
            upsample(samples, ratio, out_len)
        }
    }
}

fn downsample(samples: &[f32], ratio: f64) -> Vec<f32> {
    let out_len = (samples.len() as f64 * ratio).round() as usize;
    let step = 1.0 / ratio;
    let mut out = Vec::with_capacity(out_len);
    for i in 0..out_len {
        let start = (i as f64 * step) as usize;
        let end = (((i + 1) as f64) * step) as usize;
        let end = end.max(start + 1).min(samples.len());
        let start = start.min(samples.len() - 1);
        let slice = &samples[start..end];
        let sum: f32 = slice.iter().sum();
        out.push(sum / slice.len() as f32);
    }
    out
}

fn upsample(samples: &[f32], ratio: f64, out_len: usize) -> Vec<f32> {
    let mut out = Vec::with_capacity(out_len);
    for i in 0..out_len {
        let pos = i as f64 / ratio;
        let lo = (pos.floor() as usize).min(samples.len() - 1);
        let hi = (lo + 1).min(samples.len() - 1);
        let t = (pos - lo as f64) as f32;
        out.push(samples[lo] * (1.0 - t) + samples[hi] * t);
    }
    out
}

/// Send interleaved samples to the default output device, blocking until they
/// have been consumed.
pub(crate) fn output(samples: &[f32], sample_rate: u32) -> Result<(), PlayError> {
    let consumed = Arc::new(AtomicUsize::new(0));
    output_counted(samples, sample_rate, consumed.clone())?;
    Ok(())
}

/// The counted variant, shared with the timing tests so they can assert every
/// sample was handed to the device exactly once.
pub(crate) fn output_counted(
    samples: &[f32],
    sample_rate: u32,
    consumed: Arc<AtomicUsize>,
) -> Result<(), PlayError> {
    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

    let host = cpal::default_host();
    let device = host.default_output_device().ok_or(PlayError::NoDevice)?;

    let supported = device
        .default_output_config()
        .map_err(|e| PlayError::Device(e.to_string()))?;

    let mut config = supported.config();
    // The device was chosen for its maximum rate, so asking for it here is
    // valid; if the device disagrees it will reject the stream and we surface
    // that rather than silently playing at the wrong speed.
    config.sample_rate = sample_rate;

    let total = samples.len();
    let shared = Arc::new(Mutex::new(SharedBuffer {
        data: samples.to_vec(),
        pos: 0,
    }));
    let done = Arc::new(AtomicBool::new(false));

    let err_fn = |e: cpal::Error| tracing::error!("output stream error: {e}");

    let stream = match supported.sample_format() {
        cpal::SampleFormat::F32 => build_stream::<f32>(&device, config, shared.clone(), done.clone(), consumed.clone(), total, err_fn)?,
        cpal::SampleFormat::I16 => build_stream::<i16>(&device, config, shared.clone(), done.clone(), consumed.clone(), total, err_fn)?,
        cpal::SampleFormat::I32 => build_stream::<i32>(&device, config, shared.clone(), done.clone(), consumed.clone(), total, err_fn)?,
        cpal::SampleFormat::U8 => build_stream::<u8>(&device, config, shared.clone(), done.clone(), consumed.clone(), total, err_fn)?,
        other => {
            return Err(PlayError::Device(format!(
                "unsupported device sample format {other:?}"
            )))
        }
    };

    stream.play().map_err(|e| PlayError::Device(e.to_string()))?;

    // Wait until the buffer is fully drained. The flag alone is not enough on
    // hosts that raise callbacks eagerly: the authoritative signal is the
    // consumed counter reaching the total, and the flag is just an early
    // optimisation for the final short callback.
    while consumed.load(Ordering::SeqCst) < total {
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    // Give the device a moment to drain the last partial buffer it is holding.
    std::thread::sleep(std::time::Duration::from_millis(50));
    drop(stream);

    Ok(())
}

struct SharedBuffer {
    data: Vec<f32>,
    pos: usize,
}

fn build_stream<T>(
    device: &cpal::Device,
    config: cpal::StreamConfig,
    shared: Arc<Mutex<SharedBuffer>>,
    done: Arc<AtomicBool>,
    consumed: Arc<AtomicUsize>,
    total: usize,
    err_fn: impl FnMut(cpal::Error) + Send + 'static,
) -> Result<cpal::Stream, PlayError>
where
    T: cpal::SizedSample + cpal::FromSample<f32>,
{
    use cpal::traits::DeviceTrait;

    device
        .build_output_stream(
            config,
            move |buffer: &mut [T], _| {
                let mut guard = shared.lock().unwrap();
                let mut filled = 0usize;
                for slot in buffer.iter_mut() {
                    match guard.data.get(guard.pos) {
                        Some(&s) => {
                            *slot = T::from_sample(s);
                            guard.pos += 1;
                            filled += 1;
                        }
                        None => {
                            // Audio is over: keep the device fed with silence so
                            // the final partial block plays out in full.
                            *slot = T::from_sample(0.0f32);
                        }
                    }
                }
                consumed.fetch_add(filled, Ordering::SeqCst);
                if guard.pos >= total || filled == 0 {
                    done.store(true, Ordering::SeqCst);
                }
            },
            err_fn,
            None,
        )
        .map_err(|e| PlayError::Device(e.to_string()))
}
