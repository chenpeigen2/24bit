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
    quality: Mutex<Quality>,
}

impl AppState {
    fn engine(&self) -> Option<Engine> {
        self.engine.lock().unwrap().clone()
    }
}

#[derive(Serialize, Clone)]
struct PlayerSnapshot {
    playing: bool,
    position: u64,
    duration: u64,
    title: Option<String>,
    artist: Option<String>,
    quality: Option<String>,
    cover: Option<String>,
}

#[derive(Serialize)]
struct Prepared {
    id: String,
    lyrics: Vec<twentyfourbit_core::lyrics::LyricLine>,
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
    let tier = parse_quality(&quality);

    let mut last = std::time::Instant::now() - std::time::Duration::from_secs(1);
    let result = state
        .client
        .fetch_cached_with_progress(&id, tier, &state.cache, |done, total| {
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
        .await
        .map_err(|e| e.to_string())?;

    let (path, detail) = result;
    let lyrics = detail
        .lrc
        .as_deref()
        .map(twentyfourbit_core::lyrics::Lyrics::parse)
        .map(|l| l.lines)
        .unwrap_or_default();

    if let Some(engine) = state.engine() {
        engine.play(path);
        engine.set_volume(1.0);
    }

    *state.current.lock().unwrap() = Some(detail);

    Ok(Prepared { id, lyrics })
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
async fn set_quality(state: State<'_, AppState>, quality: String) -> Result<(), String> {
    let tier = parse_quality(&quality);
    *state.quality.lock().unwrap() = tier;
    let mut cfg = twentyfourbit_core::Config::load().unwrap_or_default();
    cfg.quality = tier;
    cfg.save().map_err(|e| e.to_string())
}

#[tauri::command]
async fn toggle(state: State<'_, AppState>) -> Result<(), String> {
    if let Some(engine) = state.engine() {
        let playing = engine
            .snapshot()
            .transport
            .map(|t| t.phase() == twentyfourbit_player::Phase::Playing)
            .unwrap_or(false);
        if playing {
            engine.pause();
        } else {
            engine.resume();
        }
    }
    Ok(())
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
        engine.seek(std::time::Duration::ZERO);
        engine.resume();
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
            (Some(s), _) => std::time::Duration::from_secs_f64(s),
            (None, Some(f)) => engine
                .snapshot()
                .transport
                .map(|t| t.duration().mul_f64(f))
                .unwrap_or_default(),
            _ => return Ok(()),
        };
        engine.seek(position);
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
    if let Some(engine) = state.engine() {
        engine.play(std::path::PathBuf::from(path));
    }
    Ok(())
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
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "twentyfourbit=info".into()),
        )
        .init();

    tauri::Builder::default()
        .setup(|app| {
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
                engine: Mutex::new(Engine::start().ok()),
                current: Mutex::new(None),
                quality: Mutex::new(cfg.quality),
            };
            app.manage(state);

            // Push the transport state to the frontend on a timer. Polling beats
            // plumbing callbacks through the audio callback thread, and 10Hz is
            // enough for a progress bar the eye reads as continuous.
            let handle = app.handle().clone();
            std::thread::spawn(move || loop {
                std::thread::sleep(std::time::Duration::from_millis(100));
                let state = handle.state::<AppState>();
                let snapshot = match state.engine() {
                    Some(engine) => {
                        let snap = engine.snapshot();
                        let detail = state.current.lock().unwrap().clone();
                        match snap.transport {
                            Some(t) => PlayerSnapshot {
                                playing: t.phase() == twentyfourbit_player::Phase::Playing,
                                position: t.position().as_millis() as u64,
                                duration: t.duration().as_millis() as u64,
                                title: detail.as_ref().map(|d| d.name.clone()),
                                artist: detail.as_ref().map(|d| d.player.clone()),
                                quality: detail.as_ref().map(|d| d.quality.clone()),
                                cover: detail.as_ref().map(|d| d.cover.clone()),
                            },
                            None => PlayerSnapshot {
                                playing: false,
                                position: 0,
                                duration: 0,
                                title: None,
                                artist: None,
                                quality: None,
                                cover: None,
                            },
                        }
                    }
                    None => continue,
                };
                let _ = handle.emit("player", snapshot);
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            search, charts, prepare, export, library, clear_cache, settings, set_quality, toggle,
            pause, restart, seek, set_volume, play_file
        ])
        .run(tauri::generate_context!())
        .expect("error while running 24bit");
}
