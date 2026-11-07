//! Behavioural tests for resilience against a flaky origin.
//!
//! The live site intermittently resets connections under no load at all —
//! observed repeatedly while building this crate. Every endpoint used here is a
//! read, so retrying is safe and is what makes the client usable in practice.

use twentyfourbit_core::{Client, ClientBuilder};

fn fixture(name: &str) -> String {
    let path = format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("fixture {path}: {e}"))
}

/// A single connection reset must not surface to the caller: the client retries
/// and the call succeeds.
#[tokio::test]
async fn transient_reset_is_retried() {
    let server = wiremock::MockServer::start().await;

    // First attempt: the origin drops the connection (502 from the proxy is the
    // closest wiremock equivalent to a reset the client can observe).
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .and(wiremock::matchers::path("/api/player/findWeekRecord"))
        .respond_with(wiremock::ResponseTemplate::new(502))
        .up_to_n_times(1)
        .mount(&server)
        .await;

    // Subsequent attempts succeed.
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .and(wiremock::matchers::path("/api/player/findWeekRecord"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(fixture("week.json")))
        .mount(&server)
        .await;

    let client = Client::builder()
        .base_url(server.uri())
        .max_attempts(3)
        .build()
        .unwrap();

    let charts = client.weekly_charts().await.expect("retry should recover");
    assert_eq!(charts.songs[0].name, "富士山下");
}

/// Persistent failures still surface after the budget is spent, rather than
/// hanging forever.
#[tokio::test]
async fn persistent_failure_surfaces_after_retries() {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .and(wiremock::matchers::path("/api/player/findWeekRecord"))
        .respond_with(wiremock::ResponseTemplate::new(502))
        .expect(3)
        .mount(&server)
        .await;

    let client = Client::builder()
        .base_url(server.uri())
        .max_attempts(3)
        .build()
        .unwrap();

    let err = client.weekly_charts().await.unwrap_err();
    match err {
        twentyfourbit_core::Error::Http { status, .. } => assert_eq!(status, 502),
        other => panic!("expected Http(502) after exhausting retries, got {other:?}"),
    }
}

/// A rejected request (`status:false`, HTTP 200) is a *semantic* answer, not a
/// transport failure — retrying it would just repeat the same rejection.
#[tokio::test]
async fn api_rejection_is_not_retried() {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .and(wiremock::matchers::path("/api/player/findWeekRecord"))
        .respond_with(
            wiremock::ResponseTemplate::new(200).set_body_string(fixture("err_keyword.json")),
        )
        .expect(1)
        .mount(&server)
        .await;

    let client = Client::builder()
        .base_url(server.uri())
        .max_attempts(3)
        .build()
        .unwrap();

    let err = client.weekly_charts().await.unwrap_err();
    assert!(
        matches!(err, twentyfourbit_core::Error::Api { .. }),
        "API rejections must not be retried, got {err:?}"
    );
}

/// The default attempt budget is part of the contract: a caller that says
/// nothing still gets resilience.
#[test]
fn default_attempt_budget_is_greater_than_one() {
    let builder: ClientBuilder = Client::builder();
    assert!(
        builder.default_attempts() >= 2,
        "a single attempt would make the client unusable against the live site"
    );
}
