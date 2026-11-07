//! Characterise the resampler: pitch accuracy in the steady-state region, and
//! stopband rejection of above-Nyquist content.

use std::f64::consts::TAU;

fn mono(samples: &[f32]) -> Vec<f32> {
    samples.iter().step_by(2).copied().collect()
}

/// Count crossings with hysteresis, over the middle of the signal only, so the
/// filter's start-up/end transients cannot pollute the estimate.
fn freq_mid(samples: &[f32], dst: u32) -> f64 {
    let m = mono(samples);
    let lo = m.len() / 4;
    let hi = m.len() * 3 / 4;
    let seg = &m[lo..hi];
    let mut crossings = 0usize;
    let mut sign = 0i32;
    let hysteresis = 0.05f32;
    for &s in seg {
        if s > hysteresis && sign <= 0 {
            crossings += 1;
            sign = 1;
        } else if s < -hysteresis && sign >= 0 {
            sign = -1;
        }
    }
    crossings as f64 * dst as f64 / seg.len() as f64
}

fn peak_mid(samples: &[f32]) -> f32 {
    let m = mono(samples);
    let lo = m.len() / 4;
    let hi = m.len() * 3 / 4;
    m[lo..hi].iter().fold(0.0f32, |a, s| a.max(s.abs()))
}

fn sine(freq: f64, rate: u32, seconds: f64) -> Vec<f32> {
    let frames = (rate as f64 * seconds) as usize;
    let mut v = Vec::with_capacity(frames * 2);
    for i in 0..frames {
        let s = (TAU * freq * i as f64 / rate as f64).sin() as f32 * 0.5;
        v.push(s);
        v.push(s);
    }
    v
}

fn convert(src: u32, dst: u32, input: &[f32]) -> Vec<f32> {
    let mut rs = twentyfourbit_player::resampler::StreamResampler::new(src, dst, 2).unwrap();
    let mut out = Vec::new();
    rs.push(input, &mut out);
    rs.flush(&mut out);
    out
}

fn main() {
    for (src, dst, f) in [
        (192_000u32, 48_000u32, 1_000.0f64),
        (44_100, 192_000, 440.0),
        (192_000, 96_000, 1_000.0),
    ] {
        let out = convert(src, dst, &sine(f, src, 1.0));
        println!("{src}->{dst}: {f}Hz -> {:.1}Hz (peak {:.3})", freq_mid(&out, dst), peak_mid(&out));
    }

    // Above-Nyquist: 30kHz cannot exist at 48kHz output.
    let out = convert(192_000, 48_000, &sine(30_000.0, 192_000, 1.0));
    println!("30kHz -> 48k: residual peak {:.4} (input amplitude 0.5)", peak_mid(&out));
}
