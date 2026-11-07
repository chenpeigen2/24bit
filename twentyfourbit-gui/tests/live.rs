//! Tests that talk to the real 24bit.net.
//!
//! Ignored by default: they need network access, and the site is a third party
//! whose data changes. Run them explicitly to confirm the reverse-engineered API
//! still behaves the way the offline fixtures assume:
//!
//! ```sh
//! cargo test -p twentyfourbit-gui --test live -- --ignored --nocapture --test-threads=1
//! ```
//!
//! Run them serially: several tests downloading at once is rude to a site that
//! pays for its own bandwidth, and it makes the timing assertion meaningless.

use twentyfourbit_core::{Cache, Client, Quality};

fn client() -> Client {
    Client::builder()
        .timeout(std::time::Duration::from_secs(25))
        .build()
        .expect("client")
}

#[tokio::test]
#[ignore = "hits the live 24bit.net"]
async fn search_returns_results_from_both_catalogues() {
    let hits = client().search("富士山下").await.expect("search failed");
    println!("{} hits, first: {}", hits.len(), hits[0].name);

    assert!(!hits.is_empty(), "search must return something for a famous song");
    assert!(
        hits.iter().any(|h| h.name.contains("富士山下")),
        "the query term should appear in the results"
    );
    assert!(
        hits.iter().all(|h| !h.id.is_empty() && !h.name.is_empty()),
        "every row must carry an id and a title"
    );
}

#[tokio::test]
#[ignore = "hits the live 24bit.net"]
async fn charts_return_songs_and_artists() {
    let charts = client().weekly_charts().await.expect("charts failed");
    println!(
        "{} songs, {} artists, top song {} ({} plays)",
        charts.songs.len(),
        charts.artists.len(),
        charts.songs[0].name,
        charts.songs[0].plays
    );

    assert!(!charts.songs.is_empty());
    assert!(!charts.artists.is_empty());
    assert!(charts.songs[0].plays > 0);
}

#[tokio::test]
#[ignore = "hits the live 24bit.net"]
async fn detail_yields_a_working_signed_url() {
    let client = client();
    let hits = client.search("富士山下").await.expect("search failed");
    let hit = hits.first().expect("no hits");

    let detail = client
        .track(&hit.id, Quality::Master)
        .await
        .expect("detail failed");
    println!(
        "{} — {} | {} | {} | expires {:?}",
        detail.name,
        detail.player,
        detail.quality,
        detail.size,
        detail.download_url_expires_at()
    );

    assert_eq!(detail.format.to_lowercase(), "flac");
    assert!(detail.download_url.starts_with("https://"));
    assert!(
        detail.download_url_expires_at().is_some(),
        "the signed URL should carry an expiry stamp"
    );
    assert!(detail.lrc.is_some(), "the master tier ships lyrics");

    // The signature is live: a one-byte range read must succeed.
    let response = reqwest::Client::new()
        .get(&detail.download_url)
        .header(reqwest::header::RANGE, "bytes=0-0")
        .send()
        .await
        .expect("range request failed");
    assert!(
        response.status().is_success(),
        "signed URL rejected: {}",
        response.status()
    );
}

/// The whole cache path, end to end: a second resolve must not re-download.
#[tokio::test]
#[ignore = "hits the live 24bit.net"]
async fn a_cached_track_resolves_twice_without_redownloading() {
    let client = client();
    let dir = std::env::temp_dir().join(format!("24bit-live-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let cache = Cache::new(&dir, 3 * 1024 * 1024 * 1024);

    let hits = client.search("富士山下").await.expect("search failed");
    let hit = hits.first().expect("no hits");

    let started = std::time::Instant::now();
    let (first, _) = client
        .fetch_cached(&hit.id, Quality::Master, &cache)
        .await
        .expect("first fetch");
    let download_time = started.elapsed();

    let started = std::time::Instant::now();
    let (second, detail) = client
        .fetch_cached(&hit.id, Quality::Master, &cache)
        .await
        .expect("second fetch");
    let cached_time = started.elapsed();

    println!(
        "download {:?} ({} bytes) vs cached {:?}",
        download_time,
        std::fs::metadata(&first).unwrap().len(),
        cached_time
    );

    assert_eq!(first, second);
    assert!(!detail.name.is_empty(), "the sidecar supplies metadata offline");
    // A cache hit reads a sidecar and stats a file; it has no reason to take
    // any significant time. An absolute bound is more honest than a ratio here,
    // because the download half depends on the network.
    assert!(
        cached_time < std::time::Duration::from_millis(200),
        "a cache hit must be near-instant, took {cached_time:?}"
    );
    assert!(
        download_time > cached_time,
        "downloading should never be faster than reading the cache"
    );
    assert_eq!(
        cache.lookup(&twentyfourbit_core::CacheKey::new(&hit.id, Quality::Master)),
        Some(first)
    );

    let _ = std::fs::remove_dir_all(&dir);
}
