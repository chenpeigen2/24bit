//! Behavioural tests for the anti-aliasing resampler.
//!
//! A sine wave's frequency is ground truth that does not depend on this
//! implementation: count zero crossings (with hysteresis) over the steady-state
//! middle of the signal. The start-up region is skipped because a windowed-sinc
//! kernel emits a legitimate delay of near-silence there, and counting sign
//! changes in that dither-level noise would measure nothing but the noise.
//!
//! The previous implementation used a box filter, which would fail the
//! above-Nyquist test below.

use std::f64::consts::TAU;
use twentyfourbit_player::resampler::StreamResampler;

fn sine_stereo(freq: f64, rate: u32, seconds: f64) -> Vec<f32> {
    let frames = (rate as f64 * seconds) as usize;
    let mut out = Vec::with_capacity(frames * 2);
    for i in 0..frames {
        let v = (TAU * freq * i as f64 / rate as f64).sin() as f32 * 0.5;
        out.push(v);
        out.push(v);
    }
    out
}

fn left_channel(samples: &[f32]) -> Vec<f32> {
    samples.iter().step_by(2).copied().collect()
}

/// Measure the dominant frequency over the middle half of the signal, using
/// hysteresis so that residual noise cannot fake a crossing.
fn measured_freq(samples: &[f32], rate: u32) -> f64 {
    let m = left_channel(samples);
    let seg = &m[m.len() / 4..m.len() * 3 / 4];

    let mut crossings = 0usize;
    let mut positive = false;
    const HYSTERESIS: f32 = 0.05;
    for &s in seg {
        if s > HYSTERESIS && !positive {
            crossings += 1;
            positive = true;
        } else if s < -HYSTERESIS && positive {
            positive = false;
        }
    }
    crossings as f64 * rate as f64 / seg.len() as f64
}

fn peak_mid(samples: &[f32]) -> f32 {
    let m = left_channel(samples);
    m[m.len() / 4..m.len() * 3 / 4]
        .iter()
        .fold(0.0f32, |a, s| a.max(s.abs()))
}

fn convert(src: u32, dst: u32, input: &[f32]) -> Vec<f32> {
    let mut rs = StreamResampler::new(src, dst, 2).unwrap();
    let mut out = Vec::new();
    rs.push(input, &mut out);
    rs.flush(&mut out);
    out
}

#[test]
fn downsampling_192k_to_48k_preserves_a_1khz_tone() {
    let out = convert(192_000, 48_000, &sine_stereo(1_000.0, 192_000, 1.0));

    let measured = measured_freq(&out, 48_000);
    assert!(
        (measured - 1_000.0).abs() < 10.0,
        "expected ~1000Hz after downsampling, measured {measured:.1}Hz"
    );

    let out_seconds = (out.len() / 2) as f64 / 48_000.0;
    assert!(
        (out_seconds - 1.0).abs() < 0.02,
        "expected ~1.0s of output, got {out_seconds:.3}s"
    );
}

#[test]
fn upsampling_44k_to_192k_preserves_a_440hz_tone() {
    let out = convert(44_100, 192_000, &sine_stereo(440.0, 44_100, 1.0));

    let measured = measured_freq(&out, 192_000);
    assert!(
        (measured - 440.0).abs() < 5.0,
        "expected ~440Hz after upsampling, measured {measured:.1}Hz"
    );
}

/// The whole point of a windowed-sinc kernel: content above the *output*
/// Nyquist must be removed rather than folded back as an alias.
///
/// A 30kHz tone cannot exist at 48kHz output. A box filter folds it to
/// 48 - 30 = 18kHz at nearly full amplitude. A real anti-aliasing filter
/// removes it.
#[test]
fn downsampling_rejects_content_above_the_output_nyquist() {
    let out = convert(192_000, 48_000, &sine_stereo(30_000.0, 192_000, 1.0));

    let residual = peak_mid(&out);
    assert!(
        residual < 0.01,
        "30kHz content should be filtered out, but residual peak was {residual:.4} \
         (input amplitude was 0.5; a box filter would alias it to 18kHz at ~0.5)"
    );
}

#[test]
fn equal_rates_pass_through_unchanged() {
    let input = sine_stereo(1_000.0, 48_000, 0.1);
    let out = convert(48_000, 48_000, &input);

    assert_eq!(out.len(), input.len(), "passthrough must not change length");
    for (a, b) in input.iter().zip(&out) {
        assert!((a - b).abs() < 1e-6, "passthrough must not alter samples");
    }
}

#[test]
fn streaming_in_small_chunks_matches_one_big_push() {
    let input = sine_stereo(1_000.0, 192_000, 0.5);

    let one = convert(192_000, 48_000, &input);

    let mut chunked = Vec::new();
    let mut rs = StreamResampler::new(192_000, 48_000, 2).unwrap();
    for chunk in input.chunks(2 * 1_000) {
        rs.push(chunk, &mut chunked);
    }
    rs.flush(&mut chunked);

    let diff = (one.len() as i64 - chunked.len() as i64).abs();
    assert!(diff <= 4, "chunking changed the output length by {diff} samples");

    let cmp = one.len().min(chunked.len());
    let max_err = one[..cmp]
        .iter()
        .zip(&chunked[..cmp])
        .fold(0.0f32, |m, (a, b)| m.max((a - b).abs()));
    assert!(max_err < 1e-3, "chunking changed the signal, max error {max_err}");
}

#[test]
fn invalid_rates_are_rejected_rather_than_panicking() {
    assert!(StreamResampler::new(0, 48_000, 2).is_err());
    assert!(StreamResampler::new(48_000, 0, 2).is_err());
}
