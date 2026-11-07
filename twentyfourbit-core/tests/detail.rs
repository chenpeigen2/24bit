//! Behavioural tests for the track-detail seam.
//!
//! The site's `findMusicPage` endpoint requires a `type` (quality tier) and an
//! `ip` field, and returns a signed, expiring download URL. These tests pin the
//! wire contract and the derived expiry behaviour.

use twentyfourbit_core::{Client, Error, Quality};

fn fixture(name: &str) -> String {
    let path = format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("fixture {path}: {e}"))
}

const MASTER_ID: &str = "c48ed2182ee4c5554d1def46d1aacbc8";

/// The detail request must carry the documented body shape, otherwise the
/// server rejects it. Verified against the live endpoint's own validation.
#[tokio::test]
async fn detail_request_sends_type_id_and_ip() {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .and(wiremock::matchers::path("/api/music/findMusicPage"))
        .and(wiremock::matchers::body_partial_json(serde_json::json!({
            "type": "a",
            "id": MASTER_ID,
        })))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(fixture("detail_a.json")))
        .mount(&server)
        .await;

    let client = Client::builder().base_url(server.uri()).build().unwrap();
    let detail = client.track(MASTER_ID, Quality::Master).await.unwrap();

    assert_eq!(detail.name, "富士山下");
    assert_eq!(detail.player, "陈奕迅");
}

/// `ip` is required by the server but its value is never checked; the client
/// fills a placeholder so callers never have to know about it.
#[tokio::test]
async fn detail_request_always_supplies_placeholder_ip() {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .and(wiremock::matchers::path("/api/music/findMusicPage"))
        .and(wiremock::matchers::body_string_contains("\"ip\""))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(fixture("detail_a.json")))
        .expect(1)
        .mount(&server)
        .await;

    let client = Client::builder().base_url(server.uri()).build().unwrap();
    client.track(MASTER_ID, Quality::Master).await.unwrap();
}

/// Quality tiers map to the site's own `type` codes.
#[tokio::test]
async fn quality_tiers_map_to_site_codes() {
    assert_eq!(Quality::Master.code(), "a");
    assert_eq!(Quality::Surround.code(), "c");
}

/// Master quality yields the 192kHz/24bit FLAC payload with lyrics.
#[tokio::test]
async fn master_detail_exposes_lossless_metadata() {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .and(wiremock::matchers::path("/api/music/findMusicPage"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(fixture("detail_a.json")))
        .mount(&server)
        .await;

    let client = Client::builder().base_url(server.uri()).build().unwrap();
    let d = client.track(MASTER_ID, Quality::Master).await.unwrap();

    assert_eq!(d.quality, "至臻母带");
    assert_eq!(d.format, "flac");
    assert_eq!(d.size, "148.90MB");
    assert!(d.lrc.is_some(), "master tier ships lyrics");
    assert!(d.download_url.starts_with("https://iot"), "got {}", d.download_url);
}

/// The surround tier is a smaller file with no lyrics — the client must not
/// pretend otherwise.
#[tokio::test]
async fn surround_detail_has_no_lyrics() {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .and(wiremock::matchers::path("/api/music/findMusicPage"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(fixture("detail_c.json")))
        .mount(&server)
        .await;

    let client = Client::builder().base_url(server.uri()).build().unwrap();
    let d = client.track(MASTER_ID, Quality::Surround).await.unwrap();

    assert_eq!(d.quality, "高清环绕声");
    assert!(d.lrc.is_none(), "surround tier carries no lyrics");
}

/// Download URLs embed their own expiry as a `YYYYMMDDhhmmss` path segment.
/// The client must surface it so callers know the link is perishable.
#[tokio::test]
async fn download_url_expiry_is_exposed() {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .and(wiremock::matchers::path("/api/music/findMusicPage"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(fixture("detail_a.json")))
        .mount(&server)
        .await;

    let client = Client::builder().base_url(server.uri()).build().unwrap();
    let d = client.track(MASTER_ID, Quality::Master).await.unwrap();

    let expiry = d
        .download_url_expires_at()
        .expect("captured master URL carries an expiry segment");
    assert_eq!(expiry, "20261106230657");
}

/// An unknown id makes the site answer HTTP 200 with `status:false`; that is an
/// error, not an empty detail.
#[tokio::test]
async fn unknown_id_is_an_error() {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .and(wiremock::matchers::path("/api/music/findMusicPage"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(
            r#"{"status":false,"result":{"message":"服务器500错误"}}"#,
        ))
        .mount(&server)
        .await;

    let client = Client::builder().base_url(server.uri()).build().unwrap();
    let err = client.track("000000000000000000000000", Quality::Master).await.unwrap_err();

    match err {
        Error::Api { message } => assert_eq!(message, "服务器500错误"),
        other => panic!("expected Error::Api, got {other:?}"),
    }
}
