//! Tauri shell around the existing Rust core.
//!
//! The webview owns presentation; everything else — the HTTP client, the LRU
//! cache, the audio engine — is the same code the CLI uses. Commands are the
//! only seam, so the frontend cannot reach the network or the sound card any
//! other way.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::Mutex;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
use twentyfourbit_core::{Cache, Client, Quality, SearchItem, TrackDetail, WeeklyCharts};
use twentyfourbit_player::Engine;

/// Everything the commands share.
pub struct AppState {
    client: Client,
    cache: Cache,
    engine: Mutex<Option<Engine>>,
    /// Metadata for whatever is loaded, so the player bar can label it without
    /// the frontend having to remember.
    current: Mutex<Option<TrackDetail>>,
    /// Last position and phase the publisher saw, for diagnosing a stalled bar.
    last_seen: Mutex<(u64, String)>,
    /// The track being fetched right now.
    ///
    /// A master file is ~150MB, so the window between "clicked play" and "the
    /// engine has a transport" is long enough to look broken. Publishing this
    /// lets the player bar show the track and its download progress instead of
    /// falling back to "nothing playing" while the bytes arrive.
    pending: Mutex<Option<PendingTrack>>,
    quality: Mutex<Quality>,
}

impl AppState {
    fn engine(&self) -> Option<Engine> {
        self.engine.lock().unwrap().clone()
    }
}

/// A track that is being fetched but has not started playing yet.
#[derive(Serialize, Clone)]
struct PendingTrack {
    id: String,
    title: String,
    artist: String,
    cover: String,
    quality: String,
    done: u64,
    total: Option<u64>,
}

#[derive(Serialize, Clone)]
struct PlayerSnapshot {
    playing: bool,
    /// Explicit terminal state; the UI must not infer EOF from a paused bar.
    ended: bool,
    position: u64,
    duration: u64,
    title: Option<String>,
    artist: Option<String>,
    quality: Option<String>,
    cover: Option<String>,
    size: Option<String>,
    album: Option<String>,
    /// False when the audio engine could not start; the UI says so instead of
    /// showing a play button that can never do anything.
    engine_ready: bool,
    /// Set while a track is being fetched. Authoritative: the frontend does not
    /// have to guess what is happening.
    pending: Option<PendingTrack>,
}

fn player_snapshot(state: &AppState) -> PlayerSnapshot {
    let engine = state.engine();
    let ready = engine.is_some();
    let detail = state.current.lock().unwrap().clone();
    let pending = state.pending.lock().unwrap().clone();

    match engine.as_ref().and_then(|e| e.snapshot().transport) {
        Some(t) => PlayerSnapshot {
            playing: t.phase() == twentyfourbit_player::Phase::Playing,
            ended: t.phase() == twentyfourbit_player::Phase::Finished,
            position: t.position().as_millis() as u64,
            duration: t.duration().as_millis() as u64,
            title: detail.as_ref().map(|d| d.name.clone()),
            artist: detail.as_ref().map(|d| d.player.clone()),
            quality: detail.as_ref().map(|d| d.quality.clone()),
            cover: detail.as_ref().map(|d| d.cover.clone()),
            size: detail.as_ref().map(|d| d.size.clone()),
            album: detail.as_ref().map(|d| d.album.clone()),
            engine_ready: ready,
            pending,
        },
        None => PlayerSnapshot {
            playing: false,
            ended: false,
            position: 0,
            duration: 0,
            title: detail.as_ref().map(|d| d.name.clone()),
            artist: detail.as_ref().map(|d| d.player.clone()),
            quality: detail.as_ref().map(|d| d.quality.clone()),
            cover: detail.as_ref().map(|d| d.cover.clone()),
            size: detail.as_ref().map(|d| d.size.clone()),
            album: detail.as_ref().map(|d| d.album.clone()),
            engine_ready: ready,
            pending,
        },
    }
}

#[derive(Serialize)]
struct Prepared {
    id: String,
    lyrics: Vec<twentyfourbit_core::lyrics::LyricLine>,
    /// Dominant colour of the artwork as `[r, g, b]`, used to tint the
    /// interface while this track plays.
    accent: [u8; 3],
}

#[derive(Serialize)]
struct LibraryRow {
    title: String,
    subtitle: String,
    path: String,
}

#[derive(Serialize)]
struct Settings {
    quality: String,
    files: usize,
    bytes: String,
    cache_dir: String,
}

// ----------------------------------------------------------------- commands

#[tauri::command]
async fn search(state: State<'_, AppState>, keyword: String) -> Result<Vec<SearchItem>, String> {
    state
        .client
        .search(&keyword)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn charts(state: State<'_, AppState>) -> Result<WeeklyCharts, String> {
    state.client.weekly_charts().await.map_err(|e| e.to_string())
}

/// Resolve a track into the cache, reporting bytes as they arrive.
#[tauri::command]
async fn prepare(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    quality: String,
) -> Result<Prepared, String> {
    tracing::info!(id, quality, "prepare requested");
    let tier = parse_quality(&quality);

    // Label the track before the transfer starts. Without this the player bar
    // has nothing to show for the whole download, which is most of the wait.
    let key = twentyfourbit_core::CacheKey::new(&id, tier);
    let detail = match state.cache.read_meta::<TrackDetail>(&key) {
        Some(cached) => cached,
        None => state
            .client
            .track(&id, tier)
            .await
            .map_err(|e| e.to_string())?,
    };

    *state.pending.lock().unwrap() = Some(PendingTrack {
        id: id.clone(),
        title: detail.name.clone(),
        artist: detail.player.clone(),
        cover: detail.cover.clone(),
        quality: detail.quality.clone(),
        done: 0,
        total: None,
    });

    let mut last = std::time::Instant::now() - std::time::Duration::from_secs(1);
    let result = state
        .client
        .fetch_cached_with_progress(&id, tier, &state.cache, |done, total| {
            {
                let mut slot = state.pending.lock().unwrap();
                if let Some(p) = slot.as_mut() {
                    p.done = done;
                    p.total = total;
                }
            }

            let now = std::time::Instant::now();
            let finished = matches!(total, Some(t) if t > 0 && done >= t);
            if finished || now.duration_since(last) >= std::time::Duration::from_millis(120) {
                last = now;
                let _ = app.emit(
                    "progress",
                    serde_json::json!({ "id": id, "done": done, "total": total }),
                );
            }
        })
        .await;

    let fetched = match result {
        Ok(fetched) => fetched,
        Err(e) => {
            *state.pending.lock().unwrap() = None;
            return Err(e.to_string());
        }
    };

    let (path, detail) = fetched;
    let lyrics = detail
        .lrc
        .as_deref()
        .map(twentyfourbit_core::lyrics::Lyrics::parse)
        .map(|l| l.lines)
        .unwrap_or_default();

    if let Some(engine) = state.engine() {
        // Keep the user's volume across tracks; the engine owns it globally.
        engine.play(path);
    }

    let accent = cover_accent(&detail.cover).unwrap_or([70, 70, 78]);
    *state.current.lock().unwrap() = Some(detail);
    // Hand off from "fetching" to "playing"; the transport takes over from here.
    *state.pending.lock().unwrap() = None;

    Ok(Prepared { id, lyrics, accent })
}

#[tauri::command]
async fn export(
    state: State<'_, AppState>,
    id: String,
    quality: String,
) -> Result<String, String> {
    let tier = parse_quality(&quality);
    let (path, detail) = state
        .client
        .fetch_cached(&id, tier, &state.cache)
        .await
        .map_err(|e| e.to_string())?;

    let dir = dirs_audio().join("24bit");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let target = dir.join(detail.suggested_filename());
    std::fs::copy(&path, &target).map_err(|e| e.to_string())?;
    Ok(target.display().to_string())
}

#[tauri::command]
async fn library(state: State<'_, AppState>) -> Result<Vec<LibraryRow>, String> {
    Ok(scan_library(state.cache.root()))
}

#[tauri::command]
async fn clear_cache(state: State<'_, AppState>) -> Result<(), String> {
    state.cache.clear().map_err(|e| e.to_string())
}

#[tauri::command]
async fn settings(state: State<'_, AppState>) -> Result<Settings, String> {
    let bytes = state.cache.total_size().unwrap_or(0);
    Ok(Settings {
        quality: match *state.quality.lock().unwrap() {
            Quality::Master => "master".into(),
            Quality::Surround => "surround".into(),
        },
        files: scan_library(state.cache.root()).len(),
        bytes: human_bytes(bytes),
        cache_dir: state.cache.root().display().to_string(),
    })
}

#[tauri::command]
async fn get_player_snapshot(state: State<'_, AppState>) -> Result<PlayerSnapshot, String> {
    static CONNECTED: std::sync::Once = std::sync::Once::new();
    CONNECTED.call_once(|| tracing::info!("frontend snapshot polling connected"));
    Ok(player_snapshot(state.inner()))
}

#[tauri::command]
async fn set_quality(state: State<'_, AppState>, quality: String) -> Result<(), String> {
    let tier = parse_quality(&quality);
    *state.quality.lock().unwrap() = tier;
    let mut cfg = twentyfourbit_core::Config::load().unwrap_or_default();
    cfg.quality = tier;
    cfg.save().map_err(|e| e.to_string())
}

/// Seek within the current decoder, or restart it when the old decoder has
/// already reached EOF. `Transport::resume` alone cannot revive `Finished`:
/// its decoder thread has no more samples to produce.
fn play_position(engine: &twentyfourbit_player::Engine, position: std::time::Duration, resume: bool) {
    let snapshot = engine.snapshot();
    let finished = snapshot
        .transport
        .as_ref()
        .map(|t| t.phase() == twentyfourbit_player::Phase::Finished)
        .unwrap_or(false);

    if finished {
        if let Some(path) = snapshot.current {
            engine.play(path);
            if !position.is_zero() {
                // Commands are ordered: the new decoder is ready before seek.
                engine.seek(position);
            }
            return;
        }
    }

    engine.seek(position);
    if resume {
        engine.resume();
    }
}

#[tauri::command]
async fn toggle(state: State<'_, AppState>, playing: Option<bool>) -> Result<bool, String> {
    let engine = state.engine().ok_or("没有可用的音频输出设备")?;
    let phase = engine.snapshot().transport.map(|t| t.phase());
    let target = playing.unwrap_or(phase != Some(twentyfourbit_player::Phase::Playing));
    tracing::info!(target, ?phase, "toggle requested");

    match phase {
        Some(twentyfourbit_player::Phase::Finished) if target => {
            play_position(&engine, std::time::Duration::ZERO, true);
        }
        _ if target => engine.resume(),
        _ => engine.pause(),
    }
    Ok(target)
}

#[tauri::command]
async fn pause(state: State<'_, AppState>) -> Result<(), String> {
    if let Some(engine) = state.engine() {
        engine.pause();
    }
    Ok(())
}

#[tauri::command]
async fn restart(state: State<'_, AppState>) -> Result<(), String> {
    if let Some(engine) = state.engine() {
        play_position(&engine, std::time::Duration::ZERO, true);
    }
    Ok(())
}

#[tauri::command]
async fn seek(
    state: State<'_, AppState>,
    seconds: Option<f64>,
    fraction: Option<f64>,
) -> Result<(), String> {
    if let Some(engine) = state.engine() {
        let position = match (seconds, fraction) {
            (Some(s), _) if s.is_finite() && s >= 0.0 => {
                std::time::Duration::from_secs_f64(s)
            }
            (None, Some(f)) if f.is_finite() => engine
                .snapshot()
                .transport
                .map(|t| t.duration().mul_f64(f.clamp(0.0, 1.0)))
                .unwrap_or_default(),
            _ => return Ok(()),
        };
        tracing::info!(position_ms = position.as_millis() as u64, "seek requested");
        play_position(&engine, position, false);
    }
    Ok(())
}

#[tauri::command]
async fn set_volume(state: State<'_, AppState>, volume: f32) -> Result<(), String> {
    if let Some(engine) = state.engine() {
        engine.set_volume(volume);
    }
    Ok(())
}

#[tauri::command]
async fn play_file(state: State<'_, AppState>, path: String) -> Result<(), String> {
    let path = std::path::PathBuf::from(path);
    let sidecar = std::fs::read_to_string(path.with_extension("json"))
        .ok()
        .and_then(|text| serde_json::from_str::<TrackDetail>(&text).ok());
    let detail = sidecar.unwrap_or_else(|| TrackDetail {
        id: path.display().to_string(),
        cover: String::new(),
        name: path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "本地音频".into()),
        player: "本地文件".into(),
        album: String::new(),
        download_url: String::new(),
        size: human_bytes(path.metadata().map(|m| m.len()).unwrap_or(0)),
        quality: "本地音频".into(),
        format: path
            .extension()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default(),
        lrc: None,
    });

    // A transport without metadata makes the UI think nothing is loaded and
    // disables pause/seek even though audio is playing.
    *state.current.lock().unwrap() = Some(detail);
    *state.pending.lock().unwrap() = None;
    if let Some(engine) = state.engine() {
        engine.play(path);
    }
    Ok(())
}

/// Search a specific page. The site returns 30 rows per page, so the UI needs a
/// way to ask for more without losing what it already has.
#[tauri::command]
async fn search_page(
    state: State<'_, AppState>,
    keyword: String,
    page: u32,
) -> Result<Vec<SearchItem>, String> {
    state
        .client
        .search_page(&keyword, page)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn login(
    state: State<'_, AppState>,
    account: String,
    password: String,
) -> Result<String, String> {
    let token = state
        .client
        .login(&account, &password)
        .await
        .map_err(|e| e.to_string())?;

    let mut cfg = twentyfourbit_core::Config::load().unwrap_or_default();
    cfg.token = Some(token);
    cfg.save().map_err(|e| e.to_string())?;
    Ok(account)
}

#[tauri::command]
async fn whoami(state: State<'_, AppState>) -> Result<Option<String>, String> {
    match state.client.whoami().await {
        Ok(account) => Ok(account.map(|a| a.account)),
        // A rejected token is not an error the UI needs to shout about; it just
        // means "not signed in".
        Err(twentyfourbit_core::Error::InvalidToken { .. }) => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}

/// Delete one cached track, both the audio and its metadata sidecar.
#[tauri::command]
async fn delete_track(state: State<'_, AppState>, path: String) -> Result<(), String> {
    let path = std::path::PathBuf::from(path);
    let cache_root = state.cache.root().to_path_buf();

    // Only ever delete inside the cache. A rendered path is not a licence to
    // hand the filesystem arbitrary input.
    if !path.starts_with(&cache_root) {
        return Err("refusing to delete a file outside the cache".into());
    }

    std::fs::remove_file(&path).map_err(|e| e.to_string())?;
    let _ = std::fs::remove_file(path.with_extension("json"));
    Ok(())
}

/// Show a cached file in Finder / Explorer / the desktop file manager.
#[tauri::command]
async fn reveal(state: State<'_, AppState>, path: String) -> Result<(), String> {
    let path = std::path::PathBuf::from(&path);
    if !path.starts_with(state.cache.root()) {
        return Err("refusing to reveal a file outside the cache".into());
    }
    if !path.exists() {
        return Err("that file is no longer cached".into());
    }

    #[cfg(target_os = "macos")]
    let status = std::process::Command::new("open")
        .args(["-R", &path.display().to_string()])
        .status();

    #[cfg(target_os = "windows")]
    let status = std::process::Command::new("explorer")
        .args(["/select,", &path.display().to_string()])
        .status();

    #[cfg(all(unix, not(target_os = "macos")))]
    let status = std::process::Command::new("xdg-open")
        .arg(path.parent().unwrap_or(std::path::Path::new(".")))
        .status();

    status.map_err(|e| e.to_string())?;
    Ok(())
}

/// Mute or unmute, returning the level that is now in effect.
#[tauri::command]
async fn mute(state: State<'_, AppState>, muted: bool, restore: f32) -> Result<f32, String> {
    let level = if muted { 0.0 } else { restore.clamp(0.0, 1.0) };
    if let Some(engine) = state.engine() {
        engine.set_volume(level);
    }
    Ok(level)
}

// ------------------------------------------------------------------ helpers

fn parse_quality(value: &str) -> Quality {
    match value {
        "surround" => Quality::Surround,
        _ => Quality::Master,
    }
}

fn dirs_audio() -> std::path::PathBuf {
    dirs::audio_dir()
        .or_else(dirs::home_dir)
        .unwrap_or_else(|| std::path::PathBuf::from("."))
}

fn scan_library(root: &std::path::Path) -> Vec<LibraryRow> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let Ok(meta) = entry.metadata() else { continue };
            if meta.is_dir() {
                stack.push(path);
                continue;
            }
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            if ext == "part" || ext == "json" {
                continue;
            }

            // The sidecar written at download time carries the real title.
            let detail: Option<TrackDetail> =
                std::fs::read_to_string(path.with_extension("json"))
                    .ok()
                    .and_then(|t| serde_json::from_str(&t).ok());

            let (title, subtitle) = match detail {
                Some(d) => (d.name, format!("{} · {}", d.player, d.quality)),
                None => (
                    path.file_stem()
                        .map(|s| s.to_string_lossy().into_owned())
                        .unwrap_or_default(),
                    String::new(),
                ),
            };
            out.push(LibraryRow {
                title,
                subtitle,
                path: path.display().to_string(),
            });
        }
    }
    out
}

/// Pull a representative colour out of a cover image.
///
/// Not the arithmetic mean — that turns most artwork into grey mud. Instead the
/// image is reduced to a small grid, near-black and near-white cells are
/// dropped, and the remaining cells are averaged in a way that favours
/// saturated pixels, because those are what a person perceives as "the colour"
/// of a cover.
fn cover_accent(url: &str) -> Option<[u8; 3]> {
    if url.is_empty() {
        return None;
    }

    let bytes = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(8))
        .build()
        .ok()?
        .get(url)
        .send()
        .ok()?
        .bytes()
        .ok()?;

    let img = image::load_from_memory(&bytes).ok()?;
    let small = img.resize_exact(24, 24, image::imageops::FilterType::Triangle);
    let rgba = small.to_rgba8();

    let mut sum = [0f32; 3];
    let mut weight = 0f32;

    for pixel in rgba.pixels() {
        let [r, g, b, a] = pixel.0;
        if a < 128 {
            continue;
        }
        let (rf, gf, bf) = (r as f32, g as f32, b as f32);
        let max = rf.max(gf).max(bf);
        let min = rf.min(gf).min(bf);

        // Skip what reads as background rather than subject.
        if max < 28.0 || min > 232.0 {
            continue;
        }

        // Saturation is the signal: a red jacket matters more than grey wall.
        let saturation = if max > 0.0 { (max - min) / max } else { 0.0 };
        let w = 0.25 + saturation * 0.75;

        sum[0] += rf * w;
        sum[1] += gf * w;
        sum[2] += bf * w;
        weight += w;
    }

    if weight <= 0.0 {
        return None;
    }

    let mut out = [
        (sum[0] / weight) as u8,
        (sum[1] / weight) as u8,
        (sum[2] / weight) as u8,
    ];

    // Nudge toward a usable UI accent: too dark reads as a shadow, too pale
    // disappears on a light chrome.
    let lum = (0.2126 * out[0] as f32 + 0.7152 * out[1] as f32 + 0.0722 * out[2] as f32) / 255.0;
    if lum < 0.22 {
        for c in &mut out {
            *c = (*c as f32 * 1.7).min(255.0) as u8;
        }
    } else if lum > 0.82 {
        for c in &mut out {
            *c = (*c as f32 * 0.82) as u8;
        }
    }

    Some(out)
}

fn human_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

// --------------------------------------------------------------------- main

fn main() {
    // Write to a file as well as stdout: a Tauri app launched from Finder has no
    // terminal, so a stalled transport would otherwise leave no trace at all.
    let log_dir = std::path::PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| "/tmp".into()))
        .join("Library/Logs/24bit");
    let _ = std::fs::create_dir_all(&log_dir);

    let file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_dir.join("app.log"))
        .ok();

    if let Some(file) = file {
        let (non_blocking, _guard) = tracing_appender::non_blocking(file);
        // Keep the guard alive for the process so the appender is not flushed and
        // dropped on the way out of `main`.
        std::mem::forget(_guard);
        let _ = tracing_subscriber::fmt()
            .with_env_filter(
                tracing_subscriber::EnvFilter::try_from_default_env()
                    .unwrap_or_else(|_| "twentyfourbit=info".into()),
            )
            .with_writer(non_blocking)
            .try_init();
    } else {
        tracing_subscriber::fmt()
            .with_env_filter(
                tracing_subscriber::EnvFilter::try_from_default_env()
                    .unwrap_or_else(|_| "twentyfourbit=info".into()),
            )
            .init();
    }

    tauri::Builder::default()
        .setup(|app| {
            // Make sure the window is actually on screen. An earlier run left
            // the process alive with nothing visible, which is indistinguishable
            // from a crash from the user's side.
            if let Some(window) = app.get_webview_window("main") {
                use tauri::PhysicalPosition;

                let _ = window.center();
                let _ = window.show();
                let _ = window.set_visible_on_all_workspaces(false);
                let _ = window.set_focus();

                // Temporary diagnostic: keep it out of fullscreen so a saved
                // position cannot push it off a disconnected display.
                let _ = window.set_fullscreen(false);

                match window.outer_position() {
                    Ok(pos) => tracing::info!("window at {pos:?}"),
                    Err(e) => tracing::warn!("cannot read window position: {e}"),
                }
                match window.outer_size() {
                    Ok(size) => tracing::info!("window size {size:?}"),
                    Err(e) => tracing::warn!("cannot read window size: {e}"),
                }

                let _ = PhysicalPosition::<i32>::new(0, 0);
            }

            let cfg = twentyfourbit_core::Config::load().unwrap_or_default();

            let mut builder = Client::builder().ip_placeholder(cfg.ip_placeholder.clone());
            if let Ok(url) = std::env::var("TWENTYFOURBIT_BASE_URL") {
                builder = builder.base_url(url);
            }
            if let Some(token) = &cfg.token {
                builder = builder.token(token.clone());
            }
            let client = builder.build()?;

            let state = AppState {
                client,
                cache: Cache::default_cache(),
                engine: Mutex::new(match Engine::start() {
                    Ok(engine) => Some(engine),
                    Err(e) => {
                        tracing::error!("audio engine unavailable: {e}");
                        None
                    }
                }),
                current: Mutex::new(None),
                pending: Mutex::new(None),
                last_seen: Mutex::new((0, String::new())),
                quality: Mutex::new(cfg.quality),
            };
            app.manage(state);

            // Push the transport state to the frontend on a timer. Polling beats
            // plumbing callbacks through the audio callback thread, and 10Hz is
            // enough for a progress bar the eye reads as continuous.
            //
            // Emitting unconditionally matters: an earlier version skipped the
            // emit whenever the audio engine was unavailable, which left the
            // window with no transport state at all and made every control look
            // dead.
            let handle = app.handle().clone();
            std::thread::spawn(move || {
                let mut last_ready: Option<bool> = None;
                let mut ticks: u64 = 0;

                loop {
                    std::thread::sleep(std::time::Duration::from_millis(100));

                    let state = handle.state::<AppState>();
                    let ready = state.engine().is_some();
                    if last_ready != Some(ready) {
                        last_ready = Some(ready);
                        tracing::info!(engine_ready = ready, "transport state publishing");
                    }
                    let snapshot = player_snapshot(state.inner());

                    let emitted = handle.emit("player", snapshot.clone()).is_ok();
                    if !emitted {
                        tracing::warn!("no listener for player events");
                    }

                    // Log changes, not every frame.
                    let state = handle.state::<AppState>();
                    let (phase, pos) = match snapshot.pending.is_some() {
                        true => ("fetching".to_owned(), snapshot.pending.as_ref().map(|p| p.done).unwrap_or(0)),
                        false => {
                            let engine = state.engine();
                            match engine.as_ref().and_then(|e| e.snapshot().transport) {
                                Some(t) => (format!("{:?}", t.phase()), t.position().as_millis() as u64),
                                None => ("none".to_owned(), 0),
                            }
                        }
                    };
                    let mut seen = state.last_seen.lock().unwrap();
                    if seen.1 != phase || (pos != seen.0 && ticks % 10 == 0) {
                        tracing::info!(phase, position = pos, "transport");
                        seen.0 = pos;
                        seen.1 = phase;
                    }
                    ticks += 1;
                }
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            search, search_page, charts, prepare, export, library, clear_cache, settings,
            set_quality, get_player_snapshot, toggle, pause, restart, seek, set_volume, play_file,
            login, whoami, delete_track, reveal, mute
        ])
        .run(tauri::generate_context!())
        .expect("error while running 24bit");
}
