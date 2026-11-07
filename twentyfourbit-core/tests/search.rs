//! Behavioural tests for the search seam.
//!
//! Expectations come from real captured server responses (see `tests/fixtures/`),
//! never from our own implementation.

use twentyfourbit_core::{Client, Error};

fn fixture(name: &str) -> String {
    let path = format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("fixture {path}: {e}"))
}

/// 24bit exposes two independent catalogues (`searchOnlineMusicOne` and
/// `...Two`). The client must query both and present one merged result set.
#[tokio::test]
async fn search_queries_both_catalogues_and_merges_them() {
    let server = wiremock::MockServer::start().await;

    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .and(wiremock::matchers::path("/api/player/searchOnlineMusicOne"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(fixture("search_one.json")))
        .expect(1)
        .mount(&server)
        .await;

    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .and(wiremock::matchers::path("/api/player/searchOnlineMusicTwo"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(fixture("search_two.json")))
        .expect(1)
        .mount(&server)
        .await;

    let client = Client::builder().base_url(server.uri()).build().unwrap();
    let results = client.search("富士山下").await.unwrap();

    let one = json_len("search_one.json");
    let two = json_len("search_two.json");

    // Both catalogues were consulted, so we hold at least as many rows as the
    // larger single catalogue returned.
    assert!(
        results.len() >= one.max(two),
        "merged result ({}) should cover the larger catalogue ({} / {})",
        results.len(),
        one,
        two
    );
    assert!(
        results.len() <= one + two,
        "merging must not invent rows: {} > {}",
        results.len(),
        one + two
    );
}

/// Duplicates across catalogues (same song, same artist) collapse into one row.
#[tokio::test]
async fn search_deduplicates_identical_songs_across_catalogues() {
    let server = wiremock::MockServer::start().await;
    let body = r#"{"status":true,"result":[
        {"id":"aaa","cover":"c","name":"富士山下","player":"陈奕迅","album":"A"},
        {"id":"bbb","cover":"c","name":"富士山下","player":"陈奕迅","album":"B"}
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
    let results = client.search("富士山下").await.unwrap();

    assert_eq!(
        results.len(),
        1,
        "same (name, player) from two catalogues must collapse to one row, got {results:#?}"
    );
    assert_eq!(results[0].id, "aaa", "the first-seen id is kept");
}

/// Search results carry the four fields the site itself renders.
#[tokio::test]
async fn search_item_exposes_site_fields() {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .and(wiremock::matchers::path("/api/player/searchOnlineMusicOne"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(fixture("search_one.json")))
        .mount(&server)
        .await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .and(wiremock::matchers::path("/api/player/searchOnlineMusicTwo"))
        .respond_with(
            wiremock::ResponseTemplate::new(200).set_body_string(r#"{"status":true,"result":[]}"#),
        )
        .mount(&server)
        .await;

    let client = Client::builder().base_url(server.uri()).build().unwrap();
    let results = client.search("富士山下").await.unwrap();

    // Ground truth: the site's own first row for this query.
    assert_eq!(results[0].id, "c48ed2182ee4c5554d1def46d1aacbc8");
    assert_eq!(results[0].name, "富士山下");
    assert_eq!(results[0].player, "陈奕迅");
    assert_eq!(results[0].album, "What's Going On…?");
}

/// A keyword with no hits is an empty list, not an error.
#[tokio::test]
async fn search_without_hits_is_empty_not_error() {
    let server = wiremock::MockServer::start().await;
    for path in [
        "/api/player/searchOnlineMusicOne",
        "/api/player/searchOnlineMusicTwo",
    ] {
        wiremock::Mock::given(wiremock::matchers::method("POST"))
            .and(wiremock::matchers::path(path))
            .respond_with(
                wiremock::ResponseTemplate::new(200).set_body_string(fixture("search_empty.json")),
            )
            .mount(&server)
            .await;
    }

    let client = Client::builder().base_url(server.uri()).build().unwrap();
    let results = client.search("zzzznotexist").await.unwrap();
    assert!(results.is_empty(), "expected no hits, got {results:#?}");
}

/// Server-side validation failures surface as a typed API error carrying the
/// site's own message.
#[tokio::test]
async fn api_rejection_surfaces_server_message() {
    let server = wiremock::MockServer::start().await;
    for path in [
        "/api/player/searchOnlineMusicOne",
        "/api/player/searchOnlineMusicTwo",
    ] {
        wiremock::Mock::given(wiremock::matchers::method("POST"))
            .and(wiremock::matchers::path(path))
            .respond_with(
                wiremock::ResponseTemplate::new(200).set_body_string(fixture("err_keyword.json")),
            )
            .mount(&server)
            .await;
    }

    let client = Client::builder().base_url(server.uri()).build().unwrap();
    let err = client.search("x").await.unwrap_err();

    match err {
        Error::Api { message } => assert!(
            message.contains("page不能为空"),
            "expected the site's message, got {message:?}"
        ),
        other => panic!("expected Error::Api, got {other:?}"),
    }
}

fn json_len(name: &str) -> usize {
    let v: serde_json::Value = serde_json::from_str(&fixture(name)).unwrap();
    v["result"].as_array().unwrap().len()
}
