//! # twentyfourbit-core
//!
//! An unofficial Rust client for [24bit.net](https://www.24bit.net), a Chinese
//! Hi-Res lossless music site that offers direct-link FLAC downloads without an
//! intermediate net-disk.
//!
//! The site ships no public API documentation. Everything in this crate was
//! recovered from its own JavaScript bundle and verified against the live
//! service; the behaviour is pinned by tests whose expectations come from
//! captured server responses rather than from this implementation.
//!
//! ## What the site actually exposes
//!
//! * `POST /api/player/searchOnlineMusicOne` and `...Two` — two *independent*
//!   catalogues, 30 rows per page.
//! * `POST /api/music/findMusicPage` — full detail plus a signed download URL.
//! * `POST /api/player/findWeekRecord` — weekly song and artist charts.
//! * `POST /api/user/login` / `getUserStatus` — optional account state.
//!
//! Two quirks worth knowing about:
//!
//! * `findMusicPage` demands an `ip` field but never validates its value, so the
//!   client fills a placeholder.
//! * The CDN serves FLAC with `content-type: audio/mpeg`. Never derive a
//!   filename from the response headers; use [`TrackDetail::suggested_filename`].
//!
//! ## Example
//!
//! ```no_run
//! # async fn run() -> twentyfourbit_core::Result<()> {
//! use twentyfourbit_core::{Client, Quality};
//!
//! let client = Client::new()?;
//! let hits = client.search("富士山下").await?;
//! let best = &hits[0];
//!
//! let detail = client.track(&best.id, Quality::Master).await?;
//! println!("{} — {} ({})", detail.name, detail.player, detail.size);
//! # Ok(())
//! # }
//! ```

pub mod cache;
pub mod client;
pub mod config;
pub mod error;
pub mod model;

pub use client::{file_extension_for, part_path, Client, ClientBuilder, DEFAULT_BASE_URL};
pub use cache::{Cache, CacheKey};
pub use config::Config;
pub use error::{Error, Result};
pub use model::{
    Account, ChartEntry, ProgressFn, Quality, SearchItem, TrackDetail, WeeklyCharts,
};
