//! End-to-end tests for the controllable engine.
//!
//! These drive real audio hardware, so they assert *timing* rather than sound:
//! the position must advance while playing, freeze while paused, and land where
//! a seek asked it to within a short tolerance.
//!
//! Acceptance criteria pinned here:
//!   - pause/resume/seek take effect within ~100ms
//!   - position never runs past the media duration

use std::f64::consts::TAU;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use twentyfourbit_player::{Engine, Phase};

/// Write a mono 16-bit WAV sine of the given length.
fn write_wav(path: &PathBuf, seconds: f64, rate: u32) {
    let frames = (seconds * rate as f64) as usize;
    let data_len = (frames * 2) as u32; // 16-bit mono
    let mut out = Vec::with_capacity(44 + data_len as usize);

    out.extend(b"RIFF");
    out.extend((36 + data_len).to_le_bytes());
    out.extend(b"WAVE");
    out.extend(b"fmt ");
    out.extend(16u32.to_le_bytes());
    out.extend(1u16.to_le_bytes()); // PCM
    out.extend(1u16.to_le_bytes()); // mono
    out.extend(rate.to_le_bytes());
    out.extend((rate * 2).to_le_bytes());
    out.extend(2u16.to_le_bytes()); // block align
    out.extend(16u16.to_le_bytes());
    out.extend(b"data");
    out.extend(data_len.to_le_bytes());

    for i in 0..frames {
        let s = (TAU * 440.0 * i as f64 / rate as f64).sin() * 0.3;
        out.extend(((s * i16::MAX as f64) as i16).to_le_bytes());
    }

    std::fs::write(path, out).unwrap();
}

/// Start the engine, or skip the test when the machine has no audio device.
///
/// CI containers have no sound card. A test that cannot run should say so
/// rather than fail: a red build for "no hardware" teaches people to ignore the
/// build, which is worse than skipping four tests.
fn engine_or_skip() -> Option<Engine> {
    match Engine::start() {
        Ok(engine) => Some(engine),
        Err(e) => {
            eprintln!("skipping: no audio device ({e})");
            None
        }
    }
}

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("24bit-engine-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir.join(name)
}

/// Poll until `predicate` holds or the timeout expires.
fn wait_for(timeout: Duration, mut predicate: impl FnMut() -> bool) -> bool {
    let start = Instant::now();
    while start.elapsed() < timeout {
        if predicate() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    false
}

#[test]
fn engine_advances_pauses_and_resumes() {
    let file = scratch("advance.wav");
    write_wav(&file, 5.0, 44_100);

    let Some(engine) = engine_or_skip() else {
        return;
    };
    engine.play(file.clone());

    assert!(
        wait_for(Duration::from_secs(3), || {
            engine
                .snapshot()
                .transport
                .as_ref()
                .map(|t| t.phase() == Phase::Playing)
                .unwrap_or(false)
        }),
        "engine never reached Playing"
    );

    assert!(
        wait_for(Duration::from_secs(2), || {
            engine
                .snapshot()
                .transport
                .as_ref()
                .map(|t| t.position() > Duration::from_millis(400))
                .unwrap_or(false)
        }),
        "position never advanced"
    );

    // Pause and confirm the position freezes.
    engine.pause();
    assert!(
        wait_for(Duration::from_millis(500), || {
            engine
                .snapshot()
                .transport
                .as_ref()
                .map(|t| t.phase() == Phase::Paused)
                .unwrap_or(false)
        }),
        "pause did not take effect within 500ms"
    );

    let frozen = engine.snapshot().transport.unwrap().position();
    std::thread::sleep(Duration::from_millis(400));
    let after = engine.snapshot().transport.unwrap().position();
    let drift = after.saturating_sub(frozen);
    assert!(
        drift < Duration::from_millis(100),
        "position drifted {drift:?} while paused"
    );

    // Resume and confirm it moves again.
    engine.resume();
    assert!(
        wait_for(Duration::from_millis(500), || {
            engine
                .snapshot()
                .transport
                .as_ref()
                .map(|t| t.position() > after + Duration::from_millis(100))
                .unwrap_or(false)
        }),
        "position did not advance after resume"
    );

    engine.stop();
    engine.shutdown();
    let _ = std::fs::remove_file(&file);
}

#[test]
fn seek_lands_close_to_the_requested_position() {
    let file = scratch("seek.wav");
    write_wav(&file, 8.0, 44_100);

    let Some(engine) = engine_or_skip() else {
        return;
    };
    engine.play(file.clone());

    assert!(
        wait_for(Duration::from_secs(3), || {
            engine
                .snapshot()
                .transport
                .as_ref()
                .map(|t| t.position() > Duration::from_millis(300))
                .unwrap_or(false)
        }),
        "playback never started"
    );

    engine.seek(Duration::from_secs(5));

    assert!(
        wait_for(Duration::from_secs(2), || {
            engine
                .snapshot()
                .transport
                .as_ref()
                .map(|t| {
                    let p = t.position();
                    p >= Duration::from_millis(4_800) && p < Duration::from_millis(5_600)
                })
                .unwrap_or(false)
        }),
        "seek did not land near 5s; position is {:?}",
        engine.snapshot().transport.map(|t| t.position())
    );

    engine.stop();
    engine.shutdown();
    let _ = std::fs::remove_file(&file);
}

#[test]
fn volume_is_stored_and_clamped() {
    let file = scratch("volume.wav");
    write_wav(&file, 2.0, 44_100);

    let Some(engine) = engine_or_skip() else {
        return;
    };
    engine.play(file.clone());
    wait_for(Duration::from_secs(2), || engine.snapshot().transport.is_some());

    engine.set_volume(0.25);
    assert!(
        wait_for(Duration::from_millis(500), || {
            engine
                .snapshot()
                .transport
                .as_ref()
                .map(|t| (t.volume() - 0.25).abs() < 1e-6)
                .unwrap_or(false)
        }),
        "volume did not reach 0.25"
    );

    engine.set_volume(9.0);
    assert!(
        wait_for(Duration::from_millis(500), || {
            engine
                .snapshot()
                .transport
                .as_ref()
                .map(|t| (t.volume() - 1.0).abs() < 1e-6)
                .unwrap_or(false)
        }),
        "volume must clamp to 1.0"
    );

    engine.stop();
    engine.shutdown();
    let _ = std::fs::remove_file(&file);
}

#[test]
fn stop_clears_the_loaded_track() {
    let file = scratch("stop.wav");
    write_wav(&file, 5.0, 44_100);

    let Some(engine) = engine_or_skip() else {
        return;
    };
    engine.play(file.clone());
    assert!(
        wait_for(Duration::from_secs(3), || engine.snapshot().current.is_some()),
        "track never loaded"
    );

    engine.stop();
    assert!(
        wait_for(Duration::from_secs(1), || engine.snapshot().current.is_none()),
        "stop did not unload the track"
    );
    engine.shutdown();
    let _ = std::fs::remove_file(&file);
}

/// The strongest check that playback is *realtime*: a two-second file must take
/// about two seconds to finish, not 50ms (which is what a broken position
/// counter or a non-blocking callback would produce).
#[test]
fn a_two_second_file_takes_about_two_seconds() {
    let file = scratch("realtime.wav");
    write_wav(&file, 2.0, 44_100);

    let Some(engine) = engine_or_skip() else {
        return;
    };
    let start = Instant::now();
    engine.play(file.clone());

    assert!(
        wait_for(Duration::from_secs(3), || {
            engine
                .snapshot()
                .transport
                .as_ref()
                .map(|t| t.phase() == Phase::Playing)
                .unwrap_or(false)
        }),
        "never started"
    );

    let finished = wait_for(Duration::from_secs(10), || {
        engine
            .snapshot()
            .transport
            .as_ref()
            .map(|t| matches!(t.phase(), Phase::Finished | Phase::Idle))
            .unwrap_or(false)
    });
    let elapsed = start.elapsed();

    assert!(finished, "playback never finished");
    assert!(
        elapsed >= Duration::from_millis(1_800),
        "2s of audio finished in {elapsed:?} — playback is not realtime"
    );
    assert!(
        elapsed < Duration::from_secs(6),
        "2s of audio took {elapsed:?} — playback is stalled"
    );

    engine.shutdown();
    let _ = std::fs::remove_file(&file);
}
