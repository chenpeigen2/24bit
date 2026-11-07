//! Behavioural tests for the download seam.
//!
//! The signed URL points at a plain HTTP object store that honours `Range`
//! requests. The client must therefore be able to stream, resume, and report
//! progress without loading the whole file into memory.

use std::io::Write;
use twentyfourbit_core::{Client, Error};

fn fixture(name: &str) -> String {
    let path = format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("fixture {path}: {e}"))
}

/// The CDN mislabels FLAC as `audio/mpeg`. The client must trust the site's
/// `format` field instead of the HTTP content type.
#[test]
fn audio_extension_comes_from_format_field_not_content_type() {
    assert_eq!(twentyfourbit_core::file_extension_for("flac"), "flac");
    assert_eq!(twentyfourbit_core::file_extension_for("FLAC"), "flac");
    assert_eq!(twentyfourbit_core::file_extension_for("wav"), "wav");
    // Unknown formats fall back to a safe value rather than the mislabelled type.
    assert_eq!(twentyfourbit_core::file_extension_for("m4a"), "m4a");
}

/// Downloading writes the exact byte count the server reports, to the path the
/// caller asked for.
#[tokio::test]
async fn download_writes_the_full_object() {
    let server = wiremock::MockServer::start().await;
    let payload: Vec<u8> = (0..4096u32).map(|i| (i % 251) as u8).collect();

    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .and(wiremock::matchers::path("/obj.flac"))
        .respond_with(
            wiremock::ResponseTemplate::new(200)
                .insert_header("content-length", payload.len().to_string())
                .set_body_bytes(payload.clone()),
        )
        .mount(&server)
        .await;

    let dir = tempdir();
    let target = dir.join("out.flac");
    let client = Client::builder().base_url(server.uri()).build().unwrap();

    let written = client
        .download_to(&format!("{}/obj.flac", server.uri()), &target)
        .await
        .unwrap();

    assert_eq!(written, payload.len() as u64);
    let got = std::fs::read(&target).unwrap();
    assert_eq!(got, payload, "downloaded bytes must match the object");
}

/// Progress is reported monotonically and ends at the total.
#[tokio::test]
async fn download_reports_monotonic_progress() {
    let server = wiremock::MockServer::start().await;
    let payload: Vec<u8> = (0..8192u32).map(|i| (i % 97) as u8).collect();

    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .and(wiremock::matchers::path("/obj.flac"))
        .respond_with(
            wiremock::ResponseTemplate::new(200)
                .insert_header("content-length", payload.len().to_string())
                .set_body_bytes(payload.clone()),
        )
        .mount(&server)
        .await;

    let dir = tempdir();
    let target = dir.join("out.flac");
    let client = Client::builder().base_url(server.uri()).build().unwrap();

    let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::<(u64, Option<u64>)>::new()));
    let sink = seen.clone();

    client
        .download_to_with_progress(
            &format!("{}/obj.flac", server.uri()),
            &target,
            move |done, total| sink.lock().unwrap().push((done, total)),
        )
        .await
        .unwrap();

    let events = seen.lock().unwrap().clone();
    assert!(!events.is_empty(), "progress must be reported at least once");

    let mut last = 0;
    for (done, _) in &events {
        assert!(*done >= last, "progress went backwards: {done} after {last}");
        last = *done;
    }
    assert_eq!(
        events.last().unwrap().0,
        payload.len() as u64,
        "final progress must equal the object size"
    );
    assert_eq!(
        events.last().unwrap().1,
        Some(payload.len() as u64),
        "total must be known from content-length"
    );
}

/// A resumed download continues from the existing partial file instead of
/// restarting, using a `Range` request.
#[tokio::test]
async fn download_resumes_from_partial_file() {
    let server = wiremock::MockServer::start().await;
    let payload: Vec<u8> = (0..4096u32).map(|i| (i % 251) as u8).collect();
    let already = 1024usize;

    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .and(wiremock::matchers::path("/obj.flac"))
        .and(wiremock::matchers::header("range", format!("bytes={already}-")))
        .respond_with(
            wiremock::ResponseTemplate::new(206)
                .insert_header("content-range", format!("bytes {already}-{}/{}", payload.len() - 1, payload.len()))
                .insert_header("content-length", (payload.len() - already).to_string())
                .set_body_bytes(payload[already..].to_vec()),
        )
        .expect(1)
        .mount(&server)
        .await;

    let dir = tempdir();
    let target = dir.join("out.flac");
    // Partial data lives in the sibling `.part` file, which is what an
    // interrupted run leaves behind.
    let mut f = std::fs::File::create(twentyfourbit_core::part_path(&target)).unwrap();
    f.write_all(&payload[..already]).unwrap();
    drop(f);

    let client = Client::builder().base_url(server.uri()).build().unwrap();
    let written = client
        .download_to(&format!("{}/obj.flac", server.uri()), &target)
        .await
        .unwrap();

    assert_eq!(
        written,
        (payload.len() - already) as u64,
        "only the missing tail should be fetched"
    );
    assert_eq!(std::fs::read(&target).unwrap(), payload, "resume must yield the whole object");
}

/// A non-auth server error during download is surfaced as a plain HTTP error,
/// distinct from the expired-link case (which is always 403 from the object
/// store and must trigger a re-fetch instead).
#[tokio::test]
async fn download_http_error_is_reported() {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .and(wiremock::matchers::path("/broken.flac"))
        .respond_with(wiremock::ResponseTemplate::new(500).set_body_string("boom"))
        .mount(&server)
        .await;

    let dir = tempdir();
    let target = dir.join("out.flac");
    let client = Client::builder().base_url(server.uri()).build().unwrap();

    let err = client
        .download_to(&format!("{}/broken.flac", server.uri()), &target)
        .await
        .unwrap_err();

    match err {
        Error::Http { status, .. } => assert_eq!(status, 500),
        other => panic!("expected Error::Http(500), got {other:?}"),
    }
}

/// A stale signed URL is a distinct, actionable error: the caller should
/// re-fetch the detail instead of retrying blindly.
#[tokio::test]
async fn expired_signed_url_is_actionable() {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .and(wiremock::matchers::path("/stale.flac"))
        .respond_with(wiremock::ResponseTemplate::new(403).set_body_string("<html>403</html>"))
        .mount(&server)
        .await;

    let dir = tempdir();
    let target = dir.join("out.flac");
    let client = Client::builder().base_url(server.uri()).build().unwrap();

    let err = client
        .download_to(&format!("{}/stale.flac", server.uri()), &target)
        .await
        .unwrap_err();

    assert!(
        matches!(err, Error::ExpiredLink { .. }),
        "403 from the object store must map to ExpiredLink, got {err:?}"
    );
    assert!(
        !target.exists(),
        "a failed download must not leave a truncated file behind"
    );
}

fn tempdir() -> std::path::PathBuf {
    // A monotonic counter, not just a timestamp: two tests starting in the same
    // clock tick would otherwise share a directory, and one test's in-flight
    // `.part` file would corrupt the other's resume assertions.
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::SeqCst);

    let p = std::env::temp_dir().join(format!("24bit-test-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(&p).unwrap();
    p
}
