//! Errors surfaced by the 24bit client.
//!
//! Every variant is actionable: the caller can tell a stale signed URL from a
//! bad token from a transport failure without parsing strings.

use std::path::PathBuf;

/// Result alias used throughout the crate.
pub type Result<T> = std::result::Result<T, Error>;

/// Everything that can go wrong talking to 24bit.net.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The site answered HTTP 200 with `{"status": false}`.
    ///
    /// `message` is the site's own validation text, kept verbatim so users see
    /// the same wording the website would show them.
    #[error("{message}")]
    Api { message: String },

    /// The transport failed (DNS, TLS, timeout, connection reset).
    #[error("network error: {0}")]
    Network(#[from] reqwest::Error),

    /// The server answered with a non-2xx status.
    #[error("HTTP {status} from {url}")]
    Http { status: u16, url: String },

    /// A signed download URL expired (the object store answers 403).
    ///
    /// The only correct response is to re-fetch the track detail for a fresh
    /// URL; retrying the same URL can never succeed.
    #[error("the signed download URL expired; fetch the track again")]
    ExpiredLink { url: String },

    /// The bearer token was rejected by the server.
    #[error("token rejected by the server: {message}")]
    InvalidToken { message: String },

    /// The response body was not the JSON shape we expect.
    #[error("unexpected response shape: {0}")]
    Decode(String),

    /// Filesystem trouble while writing a download.
    #[error("io error on {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    /// The config file exists but cannot be parsed or written.
    #[error("invalid config at {path}: {source}")]
    Config {
        path: PathBuf,
        #[source]
        source: toml::de::Error,
    },

    /// The config could not be serialised back to TOML.
    #[error("cannot serialise config for {path}: {source}")]
    ConfigEncode {
        path: PathBuf,
        #[source]
        source: toml::ser::Error,
    },
}

impl Error {
    /// True when the caller should re-fetch the track detail and try again.
    pub fn is_retryable_with_fresh_link(&self) -> bool {
        matches!(self, Error::ExpiredLink { .. })
    }

    /// True when the failure is a transport hiccup that a plain retry may cure.
    ///
    /// Deliberately excludes [`Error::Api`]: a `status:false` answer is the
    /// server's considered reply and will not change on a second attempt.
    pub fn is_transient(&self) -> bool {
        match self {
            Error::Network(_) => true,
            // 5xx and 429 come from an overloaded or restarting origin.
            Error::Http { status, .. } => *status >= 500 || *status == 429,
            _ => false,
        }
    }
}
