//! The HTTP client.
//!
//! One struct owns the transport, the bearer token, and the site's quirks
//! (`ip` placeholder, dual catalogues, mislabelled content types) so that
//! callers never have to re-discover them.

use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::error::{Error, Result};
use crate::model::{
    Account, Envelope, Quality, RawUserStatus, RawWeeklyCharts, SearchItem,
    TrackDetail, WeeklyCharts,
};

/// Default origin of the site.
pub const DEFAULT_BASE_URL: &str = "https://www.24bit.net";

/// The two live catalogues. `searchOnlineMusicThree` is unreachable (it times
/// out on every request) so it is deliberately absent.
const CATALOGUE_ONE: &str = "/api/player/searchOnlineMusicOne";
const CATALOGUE_TWO: &str = "/api/player/searchOnlineMusicTwo";

/// Placeholder IP required by `findMusicPage`. The server validates presence but
/// never the value, so this is a constant rather than a lookup.
pub const IP_PLACEHOLDER: &str = "0.0.0.0";

/// Attempts made before a read is declared failed.
///
/// The live site resets connections intermittently under no load whatsoever,
/// so a single attempt is not enough to be usable. Every endpoint this client
/// calls is a read, which makes retrying safe.
pub const DEFAULT_MAX_ATTEMPTS: u32 = 3;

/// Backoff between attempts. Kept short: the failures this guards against are
/// immediate connection resets, not slow servers.
const RETRY_BACKOFF: std::time::Duration = std::time::Duration::from_millis(250);

/// Builds a [`Client`].
#[derive(Debug, Clone)]
pub struct ClientBuilder {
    base_url: String,
    token: Option<String>,
    ip_placeholder: String,
    max_attempts: u32,
}

impl Default for ClientBuilder {
    fn default() -> Self {
        Self {
            base_url: DEFAULT_BASE_URL.to_owned(),
            token: None,
            ip_placeholder: IP_PLACEHOLDER.to_owned(),
            max_attempts: DEFAULT_MAX_ATTEMPTS,
        }
    }
}

impl ClientBuilder {
    /// Point the client at a different origin (used by tests).
    pub fn base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = url.into();
        self
    }

    /// Sign requests with a bearer token obtained from [`Client::login`].
    pub fn token(mut self, token: impl Into<String>) -> Self {
        self.token = Some(token.into());
        self
    }

    /// Override the placeholder sent in `findMusicPage`'s `ip` field.
    pub fn ip_placeholder(mut self, ip: impl Into<String>) -> Self {
        self.ip_placeholder = ip.into();
        self
    }

    /// How many times to retry a read before giving up.
    pub fn max_attempts(mut self, attempts: u32) -> Self {
        self.max_attempts = attempts.max(1);
        self
    }

    /// The attempt budget this builder will use if left alone.
    pub fn default_attempts(&self) -> u32 {
        self.max_attempts
    }

    /// Finish building.
    pub fn build(self) -> Result<Client> {
        let http = reqwest::Client::builder()
            .user_agent(concat!(
                "24bit-cli/",
                env!("CARGO_PKG_VERSION"),
                " (+https://github.com/chenpeigen/24bit)"
            ))
            .build()
            .map_err(Error::Network)?;
        Ok(Client {
            http,
            base_url: self.base_url,
            token: self.token,
            ip_placeholder: self.ip_placeholder,
            max_attempts: self.max_attempts,
        })
    }
}

/// An unofficial client for 24bit.net.
#[derive(Debug, Clone)]
pub struct Client {
    http: reqwest::Client,
    base_url: String,
    token: Option<String>,
    ip_placeholder: String,
    max_attempts: u32,
}

impl Client {
    /// Start building a client.
    pub fn builder() -> ClientBuilder {
        ClientBuilder::default()
    }

    /// A guest client against the live site.
    pub fn new() -> Result<Self> {
        Self::builder().build()
    }

    /// The token this client signs with, if any.
    pub fn token(&self) -> Option<&str> {
        self.token.as_deref()
    }

    /// Attach a token to an existing client.
    pub fn with_token(mut self, token: impl Into<String>) -> Self {
        self.token = Some(token.into());
        self
    }

    // ---------------------------------------------------------------- search

    /// Search both catalogues and return one merged, de-duplicated list.
    ///
    /// The site keeps two independent catalogues; a given song often exists in
    /// only one of them, so both are always consulted. Rows are de-duplicated on
    /// `(name, player)`, which is what the site itself treats as the same track.
    pub async fn search(&self, keyword: &str) -> Result<Vec<SearchItem>> {
        let (one, two) = tokio::join!(
            self.search_catalogue(CATALOGUE_ONE, keyword),
            self.search_catalogue(CATALOGUE_TWO, keyword)
        );
        let mut rows = one?;
        rows.extend(two?);

        let mut seen = std::collections::HashSet::new();
        rows.retain(|item| seen.insert((item.name.clone(), item.player.clone())));
        Ok(rows)
    }

    /// Search a single catalogue page. Exposed for callers that want pagination.
    pub async fn search_page(&self, keyword: &str, page: u32) -> Result<Vec<SearchItem>> {
        let (one, two) = tokio::join!(
            self.search_catalogue_page(CATALOGUE_ONE, keyword, page),
            self.search_catalogue_page(CATALOGUE_TWO, keyword, page)
        );
        let mut rows = one?;
        rows.extend(two?);
        let mut seen = std::collections::HashSet::new();
        rows.retain(|item| seen.insert((item.name.clone(), item.player.clone())));
        Ok(rows)
    }

    async fn search_catalogue(&self, path: &str, keyword: &str) -> Result<Vec<SearchItem>> {
        self.search_catalogue_page(path, keyword, 1).await
    }

    async fn search_catalogue_page(
        &self,
        path: &str,
        keyword: &str,
        page: u32,
    ) -> Result<Vec<SearchItem>> {
        #[derive(Serialize)]
        struct Body<'a> {
            keyword: &'a str,
            page: u32,
        }
        self.post::<Vec<SearchItem>, _>(path, &Body { keyword, page })
            .await
    }

    // ---------------------------------------------------------------- detail

    /// Fetch full detail (including the signed download URL) for one track.
    pub async fn track(&self, id: &str, quality: Quality) -> Result<TrackDetail> {
        #[derive(Serialize)]
        struct Body<'a> {
            #[serde(rename = "type")]
            kind: &'a str,
            id: &'a str,
            ip: &'a str,
        }
        self.post::<TrackDetail, _>(
            "/api/music/findMusicPage",
            &Body {
                kind: quality.code(),
                id,
                ip: &self.ip_placeholder,
            },
        )
        .await
    }

    // ---------------------------------------------------------------- charts

    /// The weekly song and artist rankings.
    pub async fn weekly_charts(&self) -> Result<WeeklyCharts> {
        let raw: RawWeeklyCharts = self
            .post::<RawWeeklyCharts, _>("/api/player/findWeekRecord", &serde_json::json!({}))
            .await?;
        Ok(raw.into())
    }

    // --------------------------------------------------------------- account

    /// Who is signed in, if anyone. A guest is `None`, not an error.
    pub async fn whoami(&self) -> Result<Option<Account>> {
        match self
            .post::<RawUserStatus, _>("/api/user/getUserStatus", &serde_json::json!({}))
            .await
        {
            Ok(status) => Ok(status.user.map(|u| Account { account: u.account })),
            Err(Error::Api { message }) if message.contains("token") => {
                Err(Error::InvalidToken { message })
            }
            Err(other) => Err(other),
        }
    }

    /// Exchange credentials for a bearer token.
    ///
    /// The site requires no captcha for the login endpoint itself (the geetest
    /// challenge guards registration only).
    pub async fn login(&self, account: &str, password: &str) -> Result<String> {
        #[derive(Serialize)]
        struct Body<'a> {
            account: &'a str,
            password: &'a str,
        }
        #[derive(serde::Deserialize)]
        struct LoginResult {
            token: String,
        }
        let r: LoginResult = self
            .post::<LoginResult, _>(
                "/api/user/login",
                &Body {
                    account,
                    password,
                },
            )
            .await?;
        Ok(r.token)
    }

    // -------------------------------------------------------------- download

    /// Download a signed URL to `target`, resuming a partial file if present.
    pub async fn download_to(&self, url: &str, target: &Path) -> Result<u64> {
        self.download_to_with_progress(url, target, |_, _| {}).await
    }

    /// Download with a progress callback, resuming a partial file if present.
    ///
    /// `target` is written through a sibling `.part` file and renamed only on
    /// success, so an interrupted download never leaves a plausible-looking
    /// truncated track behind.
    pub async fn download_to_with_progress(
        &self,
        url: &str,
        target: &Path,
        mut progress: impl FnMut(u64, Option<u64>) + Send,
    ) -> Result<u64> {
        let part = part_path(target);
        let already = match tokio::fs::metadata(&part).await {
            Ok(m) => m.len(),
            Err(_) => 0,
        };

        let mut request = self.http.get(url);
        if already > 0 {
            request = request.header(reqwest::header::RANGE, format!("bytes={already}-"));
        }
        let response = request.send().await.map_err(Error::Network)?;
        let status = response.status();

        if status == reqwest::StatusCode::FORBIDDEN
            || status == reqwest::StatusCode::UNAUTHORIZED
        {
            let _ = tokio::fs::remove_file(&part).await;
            return Err(Error::ExpiredLink {
                url: url.to_owned(),
            });
        }
        if !status.is_success() {
            let _ = tokio::fs::remove_file(&part).await;
            return Err(Error::Http {
                status: status.as_u16(),
                url: url.to_owned(),
            });
        }

        let total = response
            .content_length()
            .map(|len| len + already)
            .or_else(|| {
                response
                    .headers()
                    .get("content-range")
                    .and_then(|v| v.to_str().ok())
                    .and_then(|v| v.rsplit('/').next())
                    .and_then(|v| v.parse::<u64>().ok())
            });

        let mut file = if already > 0 {
            tokio::fs::OpenOptions::new()
                .append(true)
                .open(&part)
                .await
                .map_err(|e| io_err(&part, e))?
        } else {
            tokio::fs::File::create(&part)
                .await
                .map_err(|e| io_err(&part, e))?
        };

        let mut written = 0u64;
        progress(already, total);

        use futures_util::StreamExt;
        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(Error::Network)?;
            use tokio::io::AsyncWriteExt;
            file.write_all(&chunk).await.map_err(|e| io_err(&part, e))?;
            written += chunk.len() as u64;
            progress(already + written, total);
        }

        use tokio::io::AsyncWriteExt;
        file.flush().await.map_err(|e| io_err(&part, e))?;
        drop(file);

        tokio::fs::rename(&part, target)
            .await
            .map_err(|e| io_err(target, e))?;

        Ok(written)
    }

    /// Fetch a track into the cache, or return the cached copy.
    ///
    /// This is the path every playback goes through. Two behaviours matter:
    ///
    /// * A cache hit performs **zero** network requests. Master-tier files are
    ///   ~150MB, so re-downloading on every play would be unusable.
    /// * A [`Error::ExpiredLink`] is retried exactly once with a freshly fetched
    ///   detail. This is the one failure where a retry is certain to help,
    ///   because the URL — not the request — is what expired.
    pub async fn fetch_cached(
        &self,
        id: &str,
        quality: Quality,
        cache: &crate::cache::Cache,
    ) -> Result<std::path::PathBuf> {
        let key = crate::cache::CacheKey::new(id, quality);

        if let Some(hit) = cache.lookup(&key) {
            tracing::debug!(path = %hit.display(), "cache hit");
            return Ok(hit);
        }

        let target = cache.path_for(&key);
        if let Some(parent) = target.parent() {
            tokio::fs::create_dir_all(parent).await.map_err(|e| Error::Io {
                path: parent.to_path_buf(),
                source: e,
            })?;
        }

        let mut detail = self.track(id, quality).await?;

        match self.download_to(&detail.download_url, &target).await {
            Ok(_) => {}
            Err(err) if err.is_retryable_with_fresh_link() => {
                tracing::info!(id, "signed URL expired; fetching a fresh one");
                detail = self.track(id, quality).await?;
                self.download_to(&detail.download_url, &target).await?;
            }
            Err(err) => return Err(err),
        }

        cache.enforce_budget().ok();
        Ok(target)
    }

    // --------------------------------------------------------------- plumbing

    async fn post<T, B>(&self, path: &str, body: &B) -> Result<T>
    where
        T: DeserializeOwned,
        B: Serialize + ?Sized,
    {
        let url = format!("{}{path}", self.base_url);
        let mut last_err = None;

        for attempt in 1..=self.max_attempts {
            if attempt > 1 {
                tracing::debug!(attempt, path, "retrying after a transport failure");
                tokio::time::sleep(RETRY_BACKOFF).await;
            }

            match self.post_once::<T, B>(&url, body).await {
                Ok(value) => return Ok(value),
                Err(err) if err.is_transient() => {
                    last_err = Some(err);
                    continue;
                }
                // Anything else is a real answer (an API rejection, a decode
                // failure) and retrying would just repeat it.
                Err(err) => return Err(err),
            }
        }

        Err(last_err.unwrap_or(Error::Http {
            status: 0,
            url,
        }))
    }

    async fn post_once<T, B>(&self, url: &str, body: &B) -> Result<T>
    where
        T: DeserializeOwned,
        B: Serialize + ?Sized,
    {
        let mut request = self
            .http
            .post(url)
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .json(body);
        if let Some(token) = &self.token {
            request = request.header("Token", token);
        }

        let response = request.send().await.map_err(Error::Network)?;
        let status = response.status();
        if !status.is_success() {
            return Err(Error::Http {
                status: status.as_u16(),
                url: url.to_owned(),
            });
        }

        let envelope: Envelope<T> = response.json().await.map_err(Error::Network)?;
        envelope.into_result()
    }
}

/// The sibling `.part` path used for atomic downloads.
pub fn part_path(target: &Path) -> PathBuf {
    let mut name = target
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "download".to_owned());
    name.push_str(".part");
    target.with_file_name(name)
}

fn io_err(path: &Path, source: std::io::Error) -> Error {
    Error::Io {
        path: path.to_path_buf(),
        source,
    }
}

/// Map the site's `format` field to a filename extension.
///
/// Deliberately does not consult the HTTP content type: the CDN serves FLAC as
/// `audio/mpeg`, so trusting the header would name every file `.mp3`.
pub fn file_extension_for(format: &str) -> String {
    let f = format.trim().trim_start_matches('.').to_ascii_lowercase();
    if f.is_empty() {
        "bin".to_owned()
    } else {
        f
    }
}
