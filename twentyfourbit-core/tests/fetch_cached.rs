//! Behavioural tests for cache-first fetching.
//!
//! Acceptance criterion: the *second* play of the same track performs zero
//! network requests. Verified by counting requests the mock server actually
//! received, not by inspecting our own bookkeeping.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use twentyfourbit_core::cache::{Cache, CacheKey};
use twentyfourbit_core::{Client, Quality};

fn scratch(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("24bit-fetch-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A track id plus detail/object endpoints, wired to a counting server.
async fn fixture_server(
    requests: Arc<AtomicUsize>,
) -> (wiremock::MockServer, Client) {
    let server = wiremock::MockServer::start().await;

    let detail = serde_json::json!({
        "status": true,
        "result": {
            "id": "track-1",
            "cover": "c",
            "name": "Song",
            "player": "Artist",
            "album": "Album",
            "url": "PLACEHOLDER",
            "size": "1MB",
            "quality": "至臻母带",
            "format": "flac",
            "lrc": null
        }
    })
    .to_string()
    .replace("PLACEHOLDER", &format!("{}/obj.flac", server.uri()));

    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .and(wiremock::matchers::path("/api/music/findMusicPage"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(detail))
        .mount(&server)
        .await;

    let counter = requests.clone();
    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .and(wiremock::matchers::path("/obj.flac"))
        .respond_with(move |_req: &wiremock::Request| {
            counter.fetch_add(1, Ordering::SeqCst);
            wiremock::ResponseTemplate::new(200)
                .insert_header("content-length", "2048")
                .set_body_bytes(vec![7u8; 2048])
        })
        .mount(&server)
        .await;

    let client = Client::builder().base_url(server.uri()).build().unwrap();
    (server, client)
}

#[tokio::test]
async fn second_fetch_uses_the_cache_without_network() {
    let requests = Arc::new(AtomicUsize::new(0));
    let (_server, client) = fixture_server(requests.clone()).await;

    let dir = scratch("second");
    let cache = Cache::new(&dir, u64::MAX);
    let key = CacheKey::new("track-1", Quality::Master);

    let first = client
        .fetch_cached("track-1", Quality::Master, &cache)
        .await
        .expect("first fetch");
    assert_eq!(
        requests.load(Ordering::SeqCst),
        1,
        "the first fetch must hit the network exactly once"
    );

    let second = client
        .fetch_cached("track-1", Quality::Master, &cache)
        .await
        .expect("second fetch");
    assert_eq!(
        requests.load(Ordering::SeqCst),
        1,
        "the second fetch must make no network request at all"
    );
    assert_eq!(first, second, "both fetches must resolve to the same file");

    let bytes = std::fs::read(&second).unwrap();
    assert_eq!(bytes.len(), 2048);
}

#[tokio::test]
async fn different_quality_tiers_are_fetched_separately() {
    let requests = Arc::new(AtomicUsize::new(0));
    let (_server, client) = fixture_server(requests.clone()).await;

    let dir = scratch("tiers");
    let cache = Cache::new(&dir, u64::MAX);

    let master = client
        .fetch_cached("track-1", Quality::Master, &cache)
        .await
        .unwrap();
    let surround = client
        .fetch_cached("track-1", Quality::Surround, &cache)
        .await
        .unwrap();

    assert_ne!(master, surround);
    assert_eq!(
        requests.load(Ordering::SeqCst),
        2,
        "one download per tier is expected"
    );
}

/// A stale signed URL is retried once with a freshly fetched detail rather than
/// surfaced to the user. This is the only failure mode where a retry is
/// *guaranteed* to help, because the URL itself is perishable.
#[tokio::test]
async fn expired_link_is_retried_with_a_fresh_url() {
    let server = wiremock::MockServer::start().await;
    let attempts = Arc::new(AtomicUsize::new(0));

    // Detail always hands back the same object path.
    let detail_body = |url: String| {
        serde_json::json!({
            "status": true,
            "result": {
                "id": "track-1", "cover": "c", "name": "Song", "player": "Artist",
                "album": "Album", "url": url, "size": "1MB",
                "quality": "至臻母带", "format": "flac", "lrc": null
            }
        })
        .to_string()
    };
    let detail = detail_body(format!("{}/obj.flac", server.uri()));

    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .and(wiremock::matchers::path("/api/music/findMusicPage"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(detail))
        .mount(&server)
        .await;

    // The object store rejects the first attempt (expired signature) and
    // accepts the second.
    let counter = attempts.clone();
    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .and(wiremock::matchers::path("/obj.flac"))
        .respond_with(move |_req: &wiremock::Request| {
            let n = counter.fetch_add(1, Ordering::SeqCst);
            if n == 0 {
                wiremock::ResponseTemplate::new(403).set_body_string("expired")
            } else {
                wiremock::ResponseTemplate::new(200)
                    .insert_header("content-length", "16")
                    .set_body_bytes(vec![1u8; 16])
            }
        })
        .mount(&server)
        .await;

    let client = Client::builder()
        .base_url(server.uri())
        // Retry of the *transport* is separate from re-fetching the URL.
        .max_attempts(1)
        .build()
        .unwrap();

    let dir = scratch("expired");
    let cache = Cache::new(&dir, u64::MAX);

    let path = client
        .fetch_cached("track-1", Quality::Master, &cache)
        .await
        .expect("a stale link should be retried transparently");

    assert_eq!(std::fs::read(&path).unwrap().len(), 16);
    assert_eq!(
        attempts.load(Ordering::SeqCst),
        2,
        "expected exactly one retry with a fresh URL"
    );
}

/// A failure that a new URL cannot fix must not be retried forever.
#[tokio::test]
async fn persistent_object_failure_surfaces() {
    let server = wiremock::MockServer::start().await;

    let detail = serde_json::json!({
        "status": true,
        "result": {
            "id": "track-1", "cover": "c", "name": "Song", "player": "Artist",
            "album": "Album", "url": format!("{}/obj.flac", server.uri()),
            "size": "1MB", "quality": "至臻母带", "format": "flac", "lrc": null
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
        .respond_with(wiremock::ResponseTemplate::new(500))
        .mount(&server)
        .await;

    let client = Client::builder()
        .base_url(server.uri())
        .max_attempts(1)
        .build()
        .unwrap();
    let dir = scratch("persistent");
    let cache = Cache::new(&dir, u64::MAX);

    let err = client
        .fetch_cached("track-1", Quality::Master, &cache)
        .await
        .unwrap_err();

    assert!(
        matches!(err, twentyfourbit_core::Error::Http { status: 500, .. }),
        "expected the underlying HTTP error, got {err:?}"
    );
    assert!(
        cache.lookup(&CacheKey::new("track-1", Quality::Master)).is_none(),
        "a failed fetch must not leave a cache entry"
    );
}

/// Cache writes are atomic: an interrupted download must not be visible as a
/// completed entry.
#[tokio::test]
async fn partial_downloads_are_not_cache_hits() {
    let dir = scratch("partial");
    let cache = Cache::new(&dir, u64::MAX);
    let key = CacheKey::new("track-9", Quality::Master);
    let path = cache.path_for(&key);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(twentyfourbit_core::part_path(&path), b"half").unwrap();

    assert!(
        cache.lookup(&key).is_none(),
        "a .part file must never be treated as a cached track"
    );
    tokio::time::sleep(Duration::from_millis(1)).await;
}
