//! The controllable playback engine.
//!
//! Layout:
//!
//! ```text
//!   GUI / CLI ──Command──▶ control thread ──▶ decoder thread ──▶ RingBuffer
//!                                                                    │
//!                              Transport (position/phase/volume) ◀────┤
//!                                                                    ▼
//!                                                        cpal output callback
//! ```
//!
//! The output callback only ever touches the ring buffer and atomics: no locks
//! that a decoder could hold for long, no allocation, no I/O. Everything slow
//! (decoding, resampling, seeking) happens on the decoder thread.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::plan::PlaybackPlan;
use crate::resampler::StreamResampler;
use crate::ring::RingBuffer;
use crate::state::{Phase, Transport};

/// How much decoded audio to keep queued ahead of the device.
const BUFFER_SECONDS: f64 = 3.0;

/// Commands the engine accepts.
#[derive(Debug, Clone)]
pub enum Command {
    /// Load and start playing a file.
    Play(PathBuf),
    /// Pause, keeping the position.
    Pause,
    /// Resume from the paused position.
    Resume,
    /// Stop and unload.
    Stop,
    /// Jump to an absolute position.
    Seek(Duration),
    /// Set volume in `0.0..=1.0`.
    SetVolume(f32),
    /// Shut the engine down and join its threads.
    Shutdown,
}

/// A point-in-time view of the engine for a UI to read every frame.
#[derive(Debug, Clone)]
pub struct Snapshot {
    /// Position, phase and volume.
    pub transport: Option<Transport>,
    /// The track currently loaded, if any.
    pub current: Option<PathBuf>,
    /// Source rate of the loaded file.
    pub source_rate: u32,
    /// Rate actually sent to the device.
    pub output_rate: u32,
    /// True when a resampler sits in the path.
    pub resampling: bool,
    /// True while the decoder is starved and the device is emitting silence.
    pub buffering: bool,
}

/// Failure modes of the engine.
#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    /// No output device could be opened.
    #[error("no audio output device is available")]
    NoDevice,
    /// The device rejected the stream.
    #[error("cannot open the output stream: {0}")]
    Device(String),
    /// The engine's threads are gone.
    #[error("the audio engine is not running")]
    Stopped,
}

/// Handle to a running engine. Cheap to clone; all clones drive the same audio.
#[derive(Clone)]
pub struct Engine {
    commands: Sender<Command>,
    shared: Arc<Shared>,
}

/// State shared with the decoder thread and the output callback.
pub(crate) struct Shared {
    ring: Mutex<RingBuffer>,
    transport: Mutex<Option<Transport>>,
    current: Mutex<Option<PathBuf>>,
    volume: AtomicU32,
    /// Frames the decoder should skip to satisfy a seek.
    /// Requested seek in **device** frames. The decoder converts it to source
    /// frames before seeking.
    seek_request: AtomicU64,
    /// Set while the decoder is filling after a seek/underrun.
    buffering: AtomicBool,
    /// Set once the decoder has produced its last sample. Playback is only
    /// finished when this is set *and* the ring has drained.
    decoder_finished: AtomicBool,
    source_rate: AtomicU32,
    output_rate: AtomicU32,
    file_channels: AtomicU32,
    resampling: AtomicBool,
    shutdown: AtomicBool,
}

impl Engine {
    /// Start the engine on the default output device.
    pub fn start() -> Result<Self, EngineError> {
        let (commands, receiver) = std::sync::mpsc::channel();

        let device_rate = crate::engine::device_rate();
        let channels = crate::engine::device_channels();
        let frames_capacity = ((device_rate.max(48_000) as f64) * BUFFER_SECONDS) as usize
            * channels.max(1) as usize;

        let shared = Arc::new(Shared {
            ring: Mutex::new(RingBuffer::new(frames_capacity)),
            transport: Mutex::new(None),
            current: Mutex::new(None),
            volume: AtomicU32::new(1.0f32.to_bits()),
            seek_request: AtomicU64::new(u64::MAX),
            buffering: AtomicBool::new(false),
            decoder_finished: AtomicBool::new(false),
            source_rate: AtomicU32::new(0),
            output_rate: AtomicU32::new(device_rate),
            file_channels: AtomicU32::new(channels.max(1) as u32),
            resampling: AtomicBool::new(false),
            shutdown: AtomicBool::new(false),
        });

        crate::engine::spawn_output(shared.clone(), device_rate, channels)?;
        spawn_control(shared.clone(), receiver);

        Ok(Self { commands, shared })
    }

    /// Send a command. Silently ignored if the engine has already stopped.
    pub fn send(&self, command: Command) {
        let _ = self.commands.send(command);
    }

    /// Read the current state for rendering.
    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            transport: self.shared.transport.lock().unwrap().clone(),
            current: self.shared.current.lock().unwrap().clone(),
            source_rate: self.shared.source_rate.load(Ordering::Relaxed),
            output_rate: self.shared.output_rate.load(Ordering::Relaxed),
            resampling: self.shared.resampling.load(Ordering::Relaxed),
            buffering: self.shared.buffering.load(Ordering::Relaxed),
        }
    }

    /// Convenience: load and play a file.
    pub fn play(&self, path: impl Into<PathBuf>) {
        self.send(Command::Play(path.into()));
    }

    /// Convenience: pause.
    pub fn pause(&self) {
        self.send(Command::Pause);
    }

    /// Convenience: resume.
    pub fn resume(&self) {
        self.send(Command::Resume);
    }

    /// Convenience: stop.
    pub fn stop(&self) {
        self.send(Command::Stop);
    }

    /// Convenience: seek.
    pub fn seek(&self, to: Duration) {
        self.send(Command::Seek(to));
    }

    /// Convenience: volume.
    pub fn set_volume(&self, v: f32) {
        self.send(Command::SetVolume(v));
    }

    /// Stop the engine and wait for its threads to finish.
    pub fn shutdown(&self) {
        let _ = self.commands.send(Command::Shutdown);
        self.shared.shutdown.store(true, Ordering::SeqCst);
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        // Only the last clone should tear the engine down.
        if Arc::strong_count(&self.shared) == 1 {
            let _ = self.commands.send(Command::Shutdown);
            self.shared.shutdown.store(true, Ordering::SeqCst);
        }
    }
}

fn spawn_control(shared: Arc<Shared>, receiver: Receiver<Command>) {
    std::thread::Builder::new()
        .name("24bit-control".into())
        .spawn(move || {
            let mut decoder: Option<DecoderHandle> = None;

            while let Ok(command) = receiver.recv() {
                match command {
                    Command::Play(path) => {
                        if let Some(d) = decoder.take() {
                            d.stop();
                        }
                        shared.decoder_finished.store(false, Ordering::SeqCst);
                        match DecoderHandle::start(path.clone(), shared.clone()) {
                            Ok(d) => decoder = Some(d),
                            Err(e) => tracing::error!("cannot play {}: {e}", path.display()),
                        }
                    }
                    Command::Pause => {
                        if let Some(t) = shared.transport.lock().unwrap().as_mut() {
                            t.pause();
                        }
                    }
                    Command::Resume => {
                        if let Some(t) = shared.transport.lock().unwrap().as_mut() {
                            t.resume();
                        }
                    }
                    Command::Stop => {
                        if let Some(d) = decoder.take() {
                            d.stop();
                        }
                        *shared.transport.lock().unwrap() = None;
                        *shared.current.lock().unwrap() = None;
                        shared.ring.lock().unwrap().clear();
                    }
                    Command::Seek(to) => {
                        // The transport's position is measured in *device*
                        // frames, so it must be converted before it can be handed
                        // to a decoder working in source frames. Using the source
                        // rate here would land a seek a factor of four off.
                        let output_rate = shared.output_rate.load(Ordering::Relaxed);
                        let source_rate = shared.source_rate.load(Ordering::Relaxed).max(1);
                        let frames = {
                            let mut guard = shared.transport.lock().unwrap();
                            match guard.as_mut() {
                                Some(t) => {
                                    t.seek(to);
                                    let device_frames = t.position().as_secs_f64()
                                        * output_rate as f64;
                                    (device_frames * source_rate as f64 / output_rate as f64) as u64
                                }
                                None => continue,
                            }
                        };
                        shared.ring.lock().unwrap().clear();
                        shared.seek_request.store(frames, Ordering::SeqCst);
                        shared.buffering.store(true, Ordering::SeqCst);
                    }
                    Command::SetVolume(v) => {
                        shared.volume.store(v.clamp(0.0, 1.0).to_bits(), Ordering::SeqCst);
                        if let Some(t) = shared.transport.lock().unwrap().as_mut() {
                            t.set_volume(v);
                        }
                    }
                    Command::Shutdown => break,
                }
            }

            if let Some(d) = decoder.take() {
                d.stop();
            }
            shared.shutdown.store(true, Ordering::SeqCst);
        })
        .ok();
}

/// Owns the decoder thread and its stop flag.
struct DecoderHandle {
    stop: Arc<AtomicBool>,
    join: Option<std::thread::JoinHandle<()>>,
}

impl DecoderHandle {
    fn start(path: PathBuf, shared: Arc<Shared>) -> Result<Self, String> {
        let stop = Arc::new(AtomicBool::new(false));
        let stop_flag = stop.clone();

        let (ready_tx, ready_rx) = std::sync::mpsc::channel::<Result<(), String>>();

        let join = std::thread::Builder::new()
            .name("24bit-decode".into())
            .spawn(move || {
                decode_loop(path, shared, stop_flag, ready_tx);
            })
            .map_err(|e| e.to_string())?;

        ready_rx
            .recv()
            .map_err(|e| e.to_string())?
            .map(|_| DecoderHandle {
                stop,
                join: Some(join),
            })
    }

    fn stop(mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

impl Drop for DecoderHandle {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
    }
}

/// Decode the file, resample it, and keep the ring buffer topped up until
/// stopped.
fn decode_loop(
    path: PathBuf,
    shared: Arc<Shared>,
    stop: Arc<AtomicBool>,
    ready: Sender<Result<(), String>>,
) {
    use symphonia::core::codecs::audio::AudioDecoderOptions;
    use symphonia::core::formats::probe::Hint;
    use symphonia::core::formats::{FormatOptions, TrackType};
    use symphonia::core::io::MediaSourceStream;
    use symphonia::core::meta::MetadataOptions;

    let opened = (|| -> Result<_, String> {
        let file = std::fs::File::open(&path).map_err(|e| e.to_string())?;
        let mss = MediaSourceStream::new(Box::new(file), Default::default());
        let mut hint = Hint::new();
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            hint.with_extension(ext);
        }
        let format = symphonia::default::get_probe()
            .probe(&hint, mss, FormatOptions::default(), MetadataOptions::default())
            .map_err(|e| e.to_string())?;
        Ok(format)
    })();

    let mut format = match opened {
        Ok(f) => f,
        Err(e) => {
            let _ = ready.send(Err(e));
            return;
        }
    };

    let Some(track) = format.default_track(TrackType::Audio) else {
        let _ = ready.send(Err("no audio track".into()));
        return;
    };
    let track_id = track.id;
    let Some(params) = track.codec_params.as_ref().and_then(|p| p.audio()).cloned() else {
        let _ = ready.send(Err("audio track has no codec parameters".into()));
        return;
    };

    let mut decoder = match symphonia::default::get_codecs()
        .make_audio_decoder(&params, &AudioDecoderOptions::default())
    {
        Ok(d) => d,
        Err(e) => {
            let _ = ready.send(Err(e.to_string()));
            return;
        }
    };

    let source_rate = params.sample_rate.unwrap_or(48_000);
    let channels = params
        .channels
        .map(|c| c.count() as u16)
        .unwrap_or(2)
        .max(1);
    let device_rate = shared.output_rate.load(Ordering::Relaxed).max(1);
    let plan = PlaybackPlan::for_source(source_rate, device_rate);

    let total = track
        .num_frames
        .map(|n| Duration::from_secs_f64(n as f64 / source_rate as f64))
        .unwrap_or(Duration::ZERO);

    let mut transport = Transport::new(total, plan.output_rate, channels);
    transport.play();

    shared.source_rate.store(source_rate, Ordering::Relaxed);
    shared.file_channels.store(channels as u32, Ordering::Relaxed);
    shared.output_rate.store(plan.output_rate, Ordering::Relaxed);
    shared.resampling.store(plan.needs_resampling(), Ordering::Relaxed);
    *shared.transport.lock().unwrap() = Some(transport);
    *shared.current.lock().unwrap() = Some(path.clone());
    shared.ring.lock().unwrap().clear();
    shared.buffering.store(true, Ordering::SeqCst);
    shared.decoder_finished.store(false, Ordering::SeqCst);

    let _ = ready.send(Ok(()));

    let mut resampler = match StreamResampler::new(source_rate, plan.output_rate, channels as usize) {
        Ok(r) => r,
        Err(e) => {
            tracing::error!("cannot build resampler: {e}");
            return;
        }
    };

    let chunk_samples = 4096 * channels as usize;
    let mut staged: Vec<f32> = Vec::with_capacity(chunk_samples * 2);
    let mut resampled: Vec<f32> = Vec::new();

    'outer: loop {
        if stop.load(Ordering::SeqCst) || shared.shutdown.load(Ordering::SeqCst) {
            break;
        }

        // Honour a pending seek by repositioning the container.
        let seek_to = shared.seek_request.swap(u64::MAX, Ordering::SeqCst);
        if seek_to != u64::MAX {
            let secs = seek_to as f64 / source_rate as f64;
            let ts = symphonia::core::units::Time::try_new(
                secs as i64,
                ((secs.fract()) * 1e9) as u32,
            )
            .unwrap_or(symphonia::core::units::Time::ZERO);
            if format.seek(
                symphonia::core::formats::SeekMode::Accurate,
                symphonia::core::formats::SeekTo::Time { time: ts, track_id: None },
            ).is_err()
            {
                tracing::warn!("seek failed, restarting from the beginning");
            }
            resampler = match StreamResampler::new(source_rate, plan.output_rate, channels as usize) {
                Ok(r) => r,
                Err(e) => {
                    tracing::error!("cannot build resampler: {e}");
                    break;
                }
            };
            staged.clear();
            resampled.clear();
        }

        // Do not decode ahead once the buffer is comfortably full.
        let playing = shared
            .transport
            .lock()
            .unwrap()
            .as_ref()
            .map(|t| t.is_playing())
            .unwrap_or(false);

        if !playing {
            std::thread::sleep(Duration::from_millis(20));
            continue;
        }

        let queued = shared.ring.lock().unwrap().available();
        if queued * 2 > shared.ring.lock().unwrap().capacity() {
            std::thread::sleep(Duration::from_millis(10));
            continue;
        }

        // Decode one packet.
        let packet = match format.next_packet() {
            Ok(Some(p)) => p,
            Ok(None) => break 'outer,
            Err(symphonia::core::errors::Error::IoError(e))
                if e.kind() == std::io::ErrorKind::UnexpectedEof =>
            {
                break 'outer
            }
            Err(symphonia::core::errors::Error::ResetRequired) => break 'outer,
            Err(e) => {
                tracing::warn!("packet read failed: {e}");
                break 'outer;
            }
        };

        if packet.track_id != track_id {
            continue;
        }

        let decoded = match decoder.decode(&packet) {
            Ok(d) => d,
            Err(symphonia::core::errors::Error::DecodeError(_)) => continue,
            Err(e) => {
                tracing::warn!("decode failed: {e}");
                break 'outer;
            }
        };

        // `copy_to_vec_interleaved` OVERWRITES the destination (symphonia 0.6),
        // so each packet is staged before being appended.
        staged.clear();
        decoded.copy_to_vec_interleaved(&mut staged);

        resampled.clear();
        resampler.push(&staged, &mut resampled);
        push_blocking(&shared, &resampled, &stop);
        shared.buffering.store(false, Ordering::Relaxed);
    }

    // Flush whatever the resampler still holds.
    resampled.clear();
    resampler.flush(&mut resampled);
    push_blocking(&shared, &resampled, &stop);

    // The ring may still hold several seconds of audio. Marking the decoder
    // done lets the output callback decide when playback has actually finished.
    shared.decoder_finished.store(true, Ordering::SeqCst);
}

/// Push samples into the ring buffer, waiting for space instead of dropping.
fn push_blocking(shared: &Arc<Shared>, mut data: &[f32], stop: &Arc<AtomicBool>) {
    while !data.is_empty() {
        if stop.load(Ordering::SeqCst) || shared.shutdown.load(Ordering::SeqCst) {
            return;
        }
        let written = shared.ring.lock().unwrap().push(data);
        if written == 0 {
            std::thread::sleep(Duration::from_millis(5));
        } else {
            data = &data[written..];
        }
    }
}

// ---------------------------------------------------------------- output side

/// Sample rate the default output device prefers.
pub(crate) fn device_rate() -> u32 {
    use cpal::traits::{DeviceTrait, HostTrait};

    let host = cpal::default_host();
    match host.default_output_device().and_then(|d| d.default_output_config().ok()) {
        Some(config) => config.sample_rate(),
        None => 48_000,
    }
}

/// Channel count of the default output device.
pub(crate) fn device_channels() -> u16 {
    use cpal::traits::{DeviceTrait, HostTrait};

    let host = cpal::default_host();
    match host.default_output_device().and_then(|d| d.default_output_config().ok()) {
        Some(config) => config.channels(),
        None => 2,
    }
}

/// Open the output stream and keep it alive until shutdown.
pub(crate) fn spawn_output(
    shared: Arc<Shared>,
    rate: u32,
    channels: u16,
) -> Result<Arc<AtomicBool>, EngineError> {
    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

    let host = cpal::default_host();
    let device = host.default_output_device().ok_or(EngineError::NoDevice)?;
    let supported = device
        .default_output_config()
        .map_err(|e| EngineError::Device(e.to_string()))?;

    let mut config = supported.config();
    config.sample_rate = rate;
    config.channels = channels;

    let stop = Arc::new(AtomicBool::new(false));
    let keep_alive = stop.clone();
    let err_fn = |e| tracing::error!("output stream error: {e}");

    let stream = match supported.sample_format() {
        cpal::SampleFormat::F32 => build_stream::<f32>(&device, config, shared, err_fn)?,
        cpal::SampleFormat::I16 => build_stream::<i16>(&device, config, shared, err_fn)?,
        cpal::SampleFormat::I32 => build_stream::<i32>(&device, config, shared, err_fn)?,
        cpal::SampleFormat::U8 => build_stream::<u8>(&device, config, shared, err_fn)?,
        other => {
            return Err(EngineError::Device(format!(
                "unsupported device sample format {other:?}"
            )))
        }
    };

    stream.play().map_err(|e| EngineError::Device(e.to_string()))?;

    // The stream must stay owned for as long as it should play.
    std::thread::Builder::new()
        .name("24bit-output".into())
        .spawn(move || {
            while !keep_alive.load(Ordering::SeqCst) {
                std::thread::sleep(Duration::from_millis(50));
            }
            drop(stream);
        })
        .map_err(|e| EngineError::Device(e.to_string()))?;

    Ok(stop)
}

fn build_stream<T>(
    device: &cpal::Device,
    config: cpal::StreamConfig,
    shared: Arc<Shared>,
    err_fn: impl FnMut(cpal::Error) + Send + 'static,
) -> Result<cpal::Stream, EngineError>
where
    T: cpal::SizedSample + cpal::FromSample<f32>,
{
    use cpal::traits::DeviceTrait;

    let device_channels = config.channels as usize;
    let mut scratch: Vec<f32> = Vec::new();

    device
        .build_output_stream(
            config,
            move |buffer: &mut [T], _| {
                let file_channels = shared.file_channels.load(Ordering::Relaxed).max(1) as usize;
                let frames = buffer.len() / device_channels.max(1);
                if frames == 0 {
                    return;
                }

                scratch.clear();
                scratch.resize(frames * file_channels, 0.0);
                let got = shared.ring.lock().unwrap().pop_into(&mut scratch);
                let got_frames = got / file_channels;

                if got_frames < frames {
                    shared.buffering.store(true, Ordering::Relaxed);
                }

                let volume = f32::from_bits(shared.volume.load(Ordering::Relaxed));

                for f in 0..frames {
                    for c in 0..device_channels {
                        let src = c.min(file_channels - 1);
                        let sample = if f < got_frames {
                            scratch[f * file_channels + src] * volume
                        } else {
                            0.0
                        };
                        buffer[f * device_channels + c] = T::from_sample(sample);
                    }
                }

                let drained = got_frames < frames
                    && shared.decoder_finished.load(Ordering::Relaxed)
                    && shared.ring.lock().unwrap().available() == 0;

                if let Some(t) = shared.transport.lock().unwrap().as_mut() {
                    if got_frames > 0 {
                        t.advance_frames(got_frames as u64);
                    }
                    if drained && t.is_playing() {
                        t.finish();
                    }
                }
            },
            err_fn,
            None,
        )
        .map_err(|e| EngineError::Device(e.to_string()))
}

// ------------------------------------------------------------------ one-shot

/// Play a file from start to finish, blocking.
///
/// Convenience wrapper around [`Engine`] for the CLI, which has no UI to drive
/// pause or seek.
pub fn play_file(path: &std::path::Path) -> Result<(), EngineError> {
    let engine = Engine::start()?;
    engine.play(path.to_path_buf());

    loop {
        let snapshot = engine.snapshot();
        match snapshot.transport.as_ref().map(|t| t.phase()) {
            Some(Phase::Playing) | Some(Phase::Paused) => {
                std::thread::sleep(Duration::from_millis(50));
            }
            _ => break,
        }
    }

    engine.shutdown();
    Ok(())
}
