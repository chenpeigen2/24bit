//! Behavioural tests for charts, account state, and configuration.

use twentyfourbit_core::{Client, Config, Error};

fn fixture(name: &str) -> String {
    let path = format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("fixture {path}: {e}"))
}

/// The weekly charts endpoint returns both song and artist rankings.
#[tokio::test]
async fn weekly_charts_returns_songs_and_artists() {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .and(wiremock::matchers::path("/api/player/findWeekRecord"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(fixture("week.json")))
        .mount(&server)
        .await;

    let client = Client::builder().base_url(server.uri()).build().unwrap();
    let charts = client.weekly_charts().await.unwrap();

    // Ground truth from the captured payload.
    assert_eq!(charts.songs.len(), 10);
    assert_eq!(charts.songs[0].name, "富士山下");
    assert!(charts.songs[0].plays > 0);
    assert_eq!(charts.artists.len(), 10);
    assert_eq!(charts.artists[0].name, "周杰伦");
    assert!(charts.artists[0].plays > charts.songs[0].plays, "artist plays aggregate songs");
}

/// A guest session is a normal state, not an error.
#[tokio::test]
async fn guest_status_is_signed_out() {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .and(wiremock::matchers::path("/api/user/getUserStatus"))
        .respond_with(
            wiremock::ResponseTemplate::new(200).set_body_string(fixture("user_status_guest.json")),
        )
        .mount(&server)
        .await;

    let client = Client::builder().base_url(server.uri()).build().unwrap();
    let who = client.whoami().await.unwrap();
    assert!(who.is_none(), "guest must report no account, got {who:?}");
}

/// A rejected token is a distinct error so the CLI can prompt for re-login.
#[tokio::test]
async fn invalid_token_is_typed() {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .and(wiremock::matchers::path("/api/user/getUserStatus"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(
            r#"{"status":false,"result":{"message":"token解析失败"}}"#,
        ))
        .mount(&server)
        .await;

    let client = Client::builder()
        .base_url(server.uri())
        .token("bogus")
        .build()
        .unwrap();
    let err = client.whoami().await.unwrap_err();

    match err {
        Error::InvalidToken { .. } => {}
        other => panic!("expected Error::InvalidToken, got {other:?}"),
    }
}

/// The bearer token travels in the `Token` header, exactly as the site sends it.
#[tokio::test]
async fn token_travels_in_token_header() {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .and(wiremock::matchers::path("/api/user/getUserStatus"))
        .and(wiremock::matchers::header("token", "secret-token"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(
            r#"{"status":true,"result":{"user":{"account":"me"}}}"#,
        ))
        .expect(1)
        .mount(&server)
        .await;

    let client = Client::builder()
        .base_url(server.uri())
        .token("secret-token")
        .build()
        .unwrap();
    client.whoami().await.unwrap();
}

/// Configuration round-trips through TOML, and defaults are sane.
#[test]
fn config_round_trips_and_has_defaults() {
    let cfg = Config::default();
    assert_eq!(cfg.quality, twentyfourbit_core::Quality::Master);
    assert_eq!(cfg.ip_placeholder, "0.0.0.0");
    assert!(cfg.token.is_none());

    let toml = toml::to_string(&cfg).unwrap();
    let back: Config = toml::from_str(&toml).unwrap();
    assert_eq!(back, cfg);
}

/// An absent config file yields defaults rather than an error.
#[test]
fn missing_config_yields_defaults() {
    let dir = std::env::temp_dir().join(format!("24bit-cfg-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("config.toml");

    let cfg = Config::load_from(&path).unwrap();
    assert_eq!(cfg, Config::default());
}

/// A corrupt config file is a typed error with the path attached.
#[test]
fn corrupt_config_is_typed_error() {
    let dir = std::env::temp_dir().join(format!("24bit-bad-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("config.toml");
    std::fs::write(&path, "quality = 3\nnot toml {{{").unwrap();

    let err = Config::load_from(&path).unwrap_err();
    match err {
        Error::Config { path: p, .. } => assert_eq!(p, path),
        other => panic!("expected Error::Config, got {other:?}"),
    }
}
