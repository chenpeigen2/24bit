//! Behavioural tests for the backend worker.
//!
//! The UI thread must never block on the network, so all I/O happens on a
//! worker thread and results arrive as events. These tests exercise the request
//! handling directly — no window, no audio device — by feeding the handler a
//! client pointed at a mock server and collecting the events it emits.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use twentyfourbit_core::cache::{Cache, CacheKey};
use twentyfourbit_core::{Client, Quality};
use twentyfourbit_gui::backend::{handle, Event, Request};

fn collect(events: Arc<Mutex<Vec<Event>>>) -> impl Fn(Event) + Send + 'static {
    move |e| events.lock().unwrap().push(e)
}

fn scratch(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("24bit-be-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

async fn search_server() -> (wiremock::MockServer, Client) {
    let server = wiremock::MockServer::start().await;
    let body = r#"{"status":true,"result":[
        {"id":"a1","cover":"c","name":"富士山下","player":"陈奕迅","album":"A"},
        {"id":"a2","cover":"c","name":"富士山下(Live)","player":"陈奕迅","album":"B"}
    ]}"#;
    for path in [
        "/api/player/searchOnlineMusicOne",
        "/api/player/searchOnlineMusicTwo",
    ] {
        wiremock::Mock::given(wiremock::matchers::method("POST"))
            .and(wiremock::matchers::path(path))
            .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(body))
            .mount(&server)
            .await;
    }
    let client = Client::builder().base_url(server.uri()).build().unwrap();
    (server, client)
}

#[tokio::test]
async fn search_emits_results() {
    let (_server, client) = search_server().await;
    let cache = Cache::new(scratch("search"), u64::MAX);
    let events = Arc::new(Mutex::new(Vec::new()));

    handle(
        &client,
        &cache,
        Request::Search("富士山下".into()),
        collect(events.clone()),
    )
    .await;

    let events = events.lock().unwrap();
    let results = events
        .iter()
        .find_map(|e| match e {
            Event::Search(items) => Some(items),
            _ => None,
        })
        .expect("a Search event should have been emitted");

    assert_eq!(results.len(), 2);
    assert_eq!(results[0].name, "富士山下");
}

/// A failing search surfaces as an error event, never as a panic or silence.
#[tokio::test]
async fn search_failure_emits_error() {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .respond_with(wiremock::ResponseTemplate::new(500))
        .mount(&server)
        .await;
    let client = Client::builder()
        .base_url(server.uri())
        .max_attempts(1)
        .build()
        .unwrap();

    let cache = Cache::new(scratch("search-fail"), u64::MAX);
    let events = Arc::new(Mutex::new(Vec::new()));

    handle(
        &client,
        &cache,
        Request::Search("x".into()),
        collect(events.clone()),
    )
    .await;

    let events = events.lock().unwrap();
    assert!(
        events.iter().any(|e| matches!(e, Event::Error(_))),
        "expected an Error event, got {events:?}"
    );
}

/// Preparing a track resolves it into the cache and reports the local path,
/// which is what the audio engine needs.
#[tokio::test]
async fn prepare_downloads_into_the_cache_and_reports_the_path() {
    let server = wiremock::MockServer::start().await;
    let detail = serde_json::json!({
        "status": true,
        "result": {
            "id": "a1", "cover": "c", "name": "富士山下", "player": "陈奕迅",
            "album": "A", "url": format!("{}/obj.flac", server.uri()),
            "size": "2.0KB", "quality": "至臻母带", "format": "flac", "lrc": null
        }
    })
    .to_string();

    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .and(wiremock::matchers::path("/api/music/findMusicPage"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(detail))
        .mount(&server)
        .await;
    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .and(wiremock::matchers::path("/obj.flac"))
        .respond_with(
            wiremock::ResponseTemplate::new(200)
                .insert_header("content-length", "2048")
                .set_body_bytes(vec![3u8; 2048]),
        )
        .mount(&server)
        .await;

    let client = Client::builder().base_url(server.uri()).build().unwrap();
    let dir = scratch("prepare");
    let cache = Cache::new(&dir, u64::MAX);
    let events = Arc::new(Mutex::new(Vec::new()));

    handle(
        &client,
        &cache,
        Request::Prepare {
            id: "a1".into(),
            quality: Quality::Master,
        },
        collect(events.clone()),
    )
    .await;

    let events = events.lock().unwrap();
    let prepared = events
        .iter()
        .find_map(|e| match e {
            Event::Prepared { path, detail, .. } => Some((path.clone(), detail.name.clone())),
            _ => None,
        })
        .expect("a Prepared event should have been emitted");

    assert!(prepared.0.exists(), "the reported path must exist");
    assert_eq!(prepared.1, "富士山下");
    assert!(
        cache.lookup(&CacheKey::new("a1", Quality::Master)).is_some(),
        "the track must be in the cache afterwards"
    );
}

/// Preparing the same track twice must not touch the network the second time.
#[tokio::test]
async fn prepare_twice_is_served_from_cache() {
    let server = wiremock::MockServer::start().await;
    let detail = serde_json::json!({
        "status": true,
        "result": {
            "id": "a1", "cover": "c", "name": "S", "player": "P", "album": "A",
            "url": format!("{}/obj.flac", server.uri()),
            "size": "1KB", "quality": "至臻母带", "format": "flac", "lrc": null
        }
    })
    .to_string();

    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .and(wiremock::matchers::path("/api/music/findMusicPage"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(detail))
        .expect(1) // exactly one detail lookup for two prepares
        .mount(&server)
        .await;
    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .and(wiremock::matchers::path("/obj.flac"))
        .respond_with(
            wiremock::ResponseTemplate::new(200)
                .insert_header("content-length", "64")
                .set_body_bytes(vec![1u8; 64]),
        )
        .mount(&server)
        .await;

    let client = Client::builder().base_url(server.uri()).build().unwrap();
    let cache = Cache::new(scratch("prepare-twice"), u64::MAX);

    for _ in 0..2 {
        let events = Arc::new(Mutex::new(Vec::new()));
        handle(
            &client,
            &cache,
            Request::Prepare {
                id: "a1".into(),
                quality: Quality::Master,
            },
            collect(events.clone()),
        )
        .await;
    }
}

/// Charts load without a search having happened first.
#[tokio::test]
async fn charts_emit_songs_and_artists() {
    let server = wiremock::MockServer::start().await;
    let body = serde_json::json!({
        "status": true,
        "result": {
            "musics": [{"_id": "富士山下", "sumCount": 10}],
            "players": [{"_id": "周杰伦", "sumCount": 99}]
        }
    })
    .to_string();
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .and(wiremock::matchers::path("/api/player/findWeekRecord"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(body))
        .mount(&server)
        .await;

    let client = Client::builder().base_url(server.uri()).build().unwrap();
    let events = Arc::new(Mutex::new(Vec::new()));
    handle(
        &client,
        &Cache::new(scratch("charts"), u64::MAX),
        Request::Charts,
        collect(events.clone()),
    )
    .await;

    let events = events.lock().unwrap();
    let charts = events
        .iter()
        .find_map(|e| match e {
            Event::Charts(c) => Some(c),
            _ => None,
        })
        .expect("charts event");
    assert_eq!(charts.songs[0].name, "富士山下");
    assert_eq!(charts.artists[0].plays, 99);
}

/// The library lists what is actually in the cache, one row per file.
#[tokio::test]
async fn library_lists_cached_tracks() {
    let dir = scratch("library");
    let cache = Cache::new(&dir, u64::MAX);

    for (id, bytes) in [("a1", 100usize), ("a2", 250)] {
        let p = cache.path_for(&CacheKey::new(id, Quality::Master));
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(&p, vec![0u8; bytes]).unwrap();
    }

    let events = Arc::new(Mutex::new(Vec::new()));
    twentyfourbit_gui::backend::list_library(&cache, collect(events.clone()));

    let events = events.lock().unwrap();
    let rows = events
        .iter()
        .find_map(|e| match e {
            Event::Library(rows) => Some(rows),
            _ => None,
        })
        .expect("library event");

    assert_eq!(rows.len(), 2);
    let total: u64 = rows.iter().map(|r| r.bytes).sum();
    assert_eq!(total, 350);
}

/// Clearing the cache empties it and reports the new size.
#[tokio::test]
async fn clearing_the_cache_reports_zero() {
    let dir = scratch("clear");
    let cache = Cache::new(&dir, u64::MAX);
    let p = cache.path_for(&CacheKey::new("a1", Quality::Master));
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(&p, vec![0u8; 512]).unwrap();

    let events = Arc::new(Mutex::new(Vec::new()));
    twentyfourbit_gui::backend::clear_cache(&cache, collect(events.clone()));

    let events = events.lock().unwrap();
    let stats = events
        .iter()
        .find_map(|e| match e {
            Event::CacheStats { bytes, .. } => Some(*bytes),
            _ => None,
        })
        .expect("cache stats event");
    assert_eq!(stats, 0);
}

/// Events are delivered in order and never lost, which the UI relies on to
/// clear its "working…" spinner.
#[tokio::test]
async fn progress_precedes_completion() {
    let server = wiremock::MockServer::start().await;
    let detail = serde_json::json!({
        "status": true,
        "result": {
            "id": "a1", "cover": "c", "name": "S", "player": "P", "album": "A",
            "url": format!("{}/obj.flac", server.uri()),
            "size": "1KB", "quality": "至臻母带", "format": "flac", "lrc": null
        }
    })
    .to_string();
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .and(wiremock::matchers::path("/api/music/findMusicPage"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(detail))
        .mount(&server)
        .await;
    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .and(wiremock::matchers::path("/obj.flac"))
        .respond_with(
            wiremock::ResponseTemplate::new(200)
                .insert_header("content-length", "4096")
                .set_body_bytes(vec![0u8; 4096]),
        )
        .mount(&server)
        .await;

    let client = Client::builder().base_url(server.uri()).build().unwrap();
    let events = Arc::new(Mutex::new(Vec::new()));
    handle(
        &client,
        &Cache::new(scratch("progress"), u64::MAX),
        Request::Prepare {
            id: "a1".into(),
            quality: Quality::Master,
        },
        collect(events.clone()),
    )
    .await;

    let events = events.lock().unwrap();
    let prepared_at = events
        .iter()
        .position(|e| matches!(e, Event::Prepared { .. }))
        .expect("prepared event");
    let first_progress = events
        .iter()
        .position(|e| matches!(e, Event::Progress { .. }));
    if let Some(p) = first_progress {
        assert!(p < prepared_at, "progress must arrive before completion");
    }
    tokio::time::sleep(Duration::from_millis(1)).await;
}

// ---------------------------------------------------------------- exporting

async fn track_server() -> (wiremock::MockServer, Client) {
    let server = wiremock::MockServer::start().await;
    let detail = serde_json::json!({
        "status": true,
        "result": {
            "id": "a1", "cover": "c", "name": "富士山下", "player": "陈奕迅",
            "album": "What's Going On…?", "url": format!("{}/obj.flac", server.uri()),
            "size": "2.0KB", "quality": "至臻母带", "format": "flac", "lrc": null
        }
    })
    .to_string();

    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .and(wiremock::matchers::path("/api/music/findMusicPage"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(detail))
        .mount(&server)
        .await;
    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .and(wiremock::matchers::path("/obj.flac"))
        .respond_with(
            wiremock::ResponseTemplate::new(200)
                .insert_header("content-length", "1024")
                .set_body_bytes(vec![9u8; 1024]),
        )
        .mount(&server)
        .await;

    let client = Client::builder().base_url(server.uri()).build().unwrap();
    (server, client)
}

/// Exporting copies the track out under a human-readable filename.
#[tokio::test]
async fn export_writes_a_named_file_to_the_destination() {
    let (_server, client) = track_server().await;
    let cache = Cache::new(scratch("export-cache"), u64::MAX);
    let dest = scratch("export-dest");
    let events = Arc::new(Mutex::new(Vec::new()));

    handle(
        &client,
        &cache,
        Request::Export {
            id: "a1".into(),
            quality: Quality::Master,
            destination: dest.clone(),
        },
        collect(events.clone()),
    )
    .await;

    let events = events.lock().unwrap();
    let path = events
        .iter()
        .find_map(|e| match e {
            Event::Exported { path, title } => {
                assert_eq!(title, "富士山下");
                Some(path.clone())
            }
            _ => None,
        })
        .expect("an Exported event should have been emitted");

    assert!(path.exists(), "the exported file must exist");
    assert_eq!(std::fs::read(&path).unwrap().len(), 1024);
    // Readable, not a track id — and illegal characters are replaced, because
    // this album title genuinely contains '?'.
    assert_eq!(
        path.file_name().unwrap().to_string_lossy(),
        "富士山下 - 陈奕迅 [What's Going On…_].flac",
        "the filename should be readable, not a track id"
    );
}

/// Exporting must not consume the cache entry: it is a copy, not a move.
#[tokio::test]
async fn export_leaves_the_cache_entry_intact() {
    let (_server, client) = track_server().await;
    let cache = Cache::new(scratch("export-keep"), u64::MAX);
    let dest = scratch("export-keep-dest");

    let events = Arc::new(Mutex::new(Vec::new()));
    handle(
        &client,
        &cache,
        Request::Export {
            id: "a1".into(),
            quality: Quality::Master,
            destination: dest,
        },
        collect(events.clone()),
    )
    .await;

    assert!(
        cache.lookup(&CacheKey::new("a1", Quality::Master)).is_some(),
        "the cache must still hold the track after exporting"
    );
}

/// A destination that does not exist yet is created rather than failing.
#[tokio::test]
async fn export_creates_missing_directories() {
    let (_server, client) = track_server().await;
    let cache = Cache::new(scratch("export-nested"), u64::MAX);
    let dest = scratch("export-nested-dest").join("a").join("b");

    let events = Arc::new(Mutex::new(Vec::new()));
    handle(
        &client,
        &cache,
        Request::Export {
            id: "a1".into(),
            quality: Quality::Master,
            destination: dest.clone(),
        },
        collect(events.clone()),
    )
    .await;

    assert!(dest.exists(), "nested destination directories must be created");
}
