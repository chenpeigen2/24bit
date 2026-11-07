//! Data shapes returned by the 24bit API.
//!
//! Field names mirror the wire format exactly so that a captured response and a
//! deserialised value can be compared side by side.

use serde::{Deserialize, Serialize};

/// One row of a search result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchItem {
    /// Opaque track id, used for the detail lookup.
    pub id: String,
    /// Cover art URL (may be a generic placeholder for whole catalogues).
    pub cover: String,
    /// Song title.
    pub name: String,
    /// Artist, as the site spells it (may include `&`-joined performers).
    pub player: String,
    /// Album title.
    pub album: String,
}

/// Audio quality tier, mapped onto the site's own `type` codes.
///
/// The site exposes three tiers through `findMusicPage`; tier `b` is currently
/// broken server-side (it answers 500 for every id), so it is not modelled.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Quality {
    /// `type=a` — 至臻母带, typically 24bit/192kHz FLAC, ships lyrics.
    Master,
    /// `type=c` — 高清环绕声, smaller FLAC, no lyrics.
    Surround,
}

impl Quality {
    /// The wire code the site expects in the `type` field.
    pub fn code(self) -> &'static str {
        match self {
            Quality::Master => "a",
            Quality::Surround => "c",
        }
    }

    /// Parse a wire code back into a tier.
    pub fn from_code(code: &str) -> Option<Self> {
        match code {
            "a" => Some(Quality::Master),
            "c" => Some(Quality::Surround),
            _ => None,
        }
    }
}

impl Default for Quality {
    fn default() -> Self {
        Quality::Master
    }
}

/// Full detail for one track at one quality tier.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackDetail {
    pub id: String,
    pub cover: String,
    pub name: String,
    pub player: String,
    pub album: String,
    /// Signed, expiring direct download URL.
    #[serde(rename = "url")]
    pub download_url: String,
    /// Human-readable size, e.g. `148.90MB`.
    pub size: String,
    /// Human-readable tier name, e.g. `至臻母带`.
    pub quality: String,
    /// Container format, e.g. `flac`.
    pub format: String,
    /// Embedded lyrics, present only on the master tier.
    #[serde(default)]
    pub lrc: Option<String>,
}

impl TrackDetail {
    /// The `YYYYMMDDhhmmss` expiry segment embedded in the signed URL path.
    ///
    /// The object store rejects the URL once this instant has passed, so callers
    /// should treat the link as perishable and re-fetch on
    /// [`crate::Error::ExpiredLink`].
    pub fn download_url_expires_at(&self) -> Option<String> {
        self.download_url
            .split('/')
            .find(|seg| seg.len() == 14 && seg.chars().all(|c| c.is_ascii_digit()))
            .map(str::to_owned)
    }

    /// A safe filename for saving this track.
    ///
    /// Built from the site's own metadata and the `format` field — never from
    /// the HTTP content type, which the CDN mislabels as `audio/mpeg`.
    pub fn suggested_filename(&self) -> String {
        let ext = crate::file_extension_for(&self.format);
        let sanitise = |s: &str| {
            s.chars()
                .map(|c| if "/\\:*?\"<>|".contains(c) { '_' } else { c })
                .collect::<String>()
        };
        format!(
            "{} - {} [{}].{}",
            sanitise(&self.name),
            sanitise(&self.player),
            sanitise(&self.album),
            ext
        )
    }
}

/// One entry in a weekly ranking.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChartEntry {
    /// Song title or artist name, depending on which list it came from.
    pub name: String,
    /// Total plays this week.
    pub plays: u64,
}

/// The weekly charts, split into songs and artists.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WeeklyCharts {
    pub songs: Vec<ChartEntry>,
    pub artists: Vec<ChartEntry>,
}

/// The signed-in account, if any.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Account {
    pub account: String,
}

/// Progress callback payload: bytes written so far and the total if known.
pub type ProgressFn = Box<dyn FnMut(u64, Option<u64>) + Send>;

/// The envelope every endpoint answers with.
#[derive(Debug, Deserialize)]
pub(crate) struct Envelope<T> {
    pub status: bool,
    pub result: serde_json::Value,
    #[serde(skip)]
    pub _marker: std::marker::PhantomData<T>,
}

impl<T> Envelope<T> {
    /// Split the envelope into a typed payload or an API error.
    pub(crate) fn into_result(self) -> crate::Result<T>
    where
        T: serde::de::DeserializeOwned,
    {
        if self.status {
            let raw = self.result;
            serde_json::from_value(raw.clone())
                .map_err(|e| crate::Error::Decode(format!("{e}: {raw}")))
        } else {
            let message = self
                .result
                .get("message")
                .and_then(|m| m.as_str())
                .unwrap_or("unknown server error")
                .to_owned();
            Err(crate::Error::Api { message })
        }
    }
}

/// Wire shape of one weekly chart row.
#[derive(Debug, Deserialize)]
pub(crate) struct RawChartRow {
    #[serde(rename = "_id")]
    pub name: String,
    #[serde(rename = "sumCount")]
    pub plays: u64,
}

/// Wire shape of the weekly charts payload.
#[derive(Debug, Deserialize)]
pub(crate) struct RawWeeklyCharts {
    pub musics: Vec<RawChartRow>,
    pub players: Vec<RawChartRow>,
}

impl From<RawWeeklyCharts> for WeeklyCharts {
    fn from(raw: RawWeeklyCharts) -> Self {
        let map = |rows: Vec<RawChartRow>| {
            rows.into_iter()
                .map(|r| ChartEntry {
                    name: r.name,
                    plays: r.plays,
                })
                .collect()
        };
        WeeklyCharts {
            songs: map(raw.musics),
            artists: map(raw.players),
        }
    }
}

/// Wire shape of the user-status payload.
#[derive(Debug, Deserialize)]
pub(crate) struct RawUserStatus {
    pub user: Option<RawUser>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct RawUser {
    pub account: String,
}
