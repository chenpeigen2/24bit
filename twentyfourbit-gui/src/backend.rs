//! The background worker that does all network and cache I/O.
//!
//! The UI thread must never block, so every request is handed to a worker that
//! owns a tokio runtime, and results come back as [`Event`]s through a channel.
//!
//! Request handling ([`handle`]) takes the client and cache as arguments rather
//! than owning them, which is what makes the whole thing testable without a
//! window or an audio device.

use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender};

use twentyfourbit_core::cache::Cache;
use twentyfourbit_core::{Client, Quality, SearchItem, TrackDetail, WeeklyCharts};

/// Work the UI asks for.
#[derive(Debug, Clone)]
pub enum Request {
    /// Search both catalogues.
    Search(String),
    /// Load the weekly charts.
    Charts,
    /// Resolve a track into the cache and report the local path.
    Prepare {
        /// Track id.
        id: String,
        /// Which tier to fetch.
        quality: Quality,
    },
    /// Copy a cached track out to a user-visible directory.
    Export {
        /// Track id.
        id: String,
        /// Which tier to fetch.
        quality: Quality,
        /// Directory to copy into.
        destination: PathBuf,
    },
    /// Re-scan the cache for the library view.
    Library,
    /// Delete every cached file.
    ClearCache,
    /// Report who is signed in.
    WhoAmI,
    /// Sign in and persist the token.
    Login {
        /// Account name.
        account: String,
        /// Password.
        password: String,
    },
}

/// One row of the library view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibraryRow {
    /// Filename, used as the display title.
    pub name: String,
    /// Local path.
    pub path: PathBuf,
    /// Size in bytes.
    pub bytes: u64,
}

/// Things the UI reacts to.
#[derive(Debug, Clone)]
pub enum Event {
    /// A request started; the UI shows a spinner.
    Busy(&'static str),
    /// Search results.
    Search(Vec<SearchItem>),
    /// Weekly charts.
    Charts(WeeklyCharts),
    /// A track is ready to play from `path`.
    Prepared {
        /// Track id.
        id: String,
        /// Local file.
        path: PathBuf,
        /// Everything the UI needs to label the track and show its lyrics.
        detail: TrackDetail,
    },
    /// A track was copied out of the cache to `path`.
    Exported {
        /// Where it landed.
        path: PathBuf,
        /// Song title, for the confirmation message.
        title: String,
    },
    /// Download progress for the current request.
    Progress {
        /// Bytes written so far.
        done: u64,
        /// Total bytes, when known.
        total: Option<u64>,
    },
    /// Cache contents.
    Library(Vec<LibraryRow>),
    /// Cache size summary.
    CacheStats {
        /// Number of cached files.
        files: usize,
        /// Total bytes.
        bytes: u64,
    },
    /// Signed-in account, if any.
    Account(Option<String>),
    /// A request failed.
    Error(String),
}

/// Do the work for one request, emitting events as it goes.
pub async fn handle(
    client: &Client,
    cache: &Cache,
    request: Request,
    emit: impl Fn(Event) + Send + 'static,
) {
    match request {
        Request::Search(keyword) => {
            emit(Event::Busy("searching"));
            match client.search(&keyword).await {
                Ok(items) => emit(Event::Search(items)),
                Err(e) => emit(Event::Error(e.to_string())),
            }
        }

        Request::Charts => {
            emit(Event::Busy("loading charts"));
            match client.weekly_charts().await {
                Ok(charts) => emit(Event::Charts(charts)),
                Err(e) => emit(Event::Error(e.to_string())),
            }
        }

        Request::Prepare { id, quality } => {
            emit(Event::Busy("preparing track"));
            match client.fetch_cached(&id, quality, cache).await {
                Ok((path, detail)) => emit(Event::Prepared { id, path, detail }),
                Err(e) => emit(Event::Error(e.to_string())),
            }
        }

        Request::Export {
            id,
            quality,
            destination,
        } => {
            emit(Event::Busy("exporting track"));
            match client.fetch_cached(&id, quality, cache).await {
                Ok((path, detail)) => {
                    let target = destination.join(detail.suggested_filename());
                    match export_file(&path, &target) {
                        Ok(()) => emit(Event::Exported {
                            path: target,
                            title: detail.name,
                        }),
                        Err(e) => emit(Event::Error(e)),
                    }
                }
                Err(e) => emit(Event::Error(e.to_string())),
            }
        }

        Request::Library => list_library(cache, emit),
        Request::ClearCache => clear_cache(cache, emit),

        Request::WhoAmI => match client.whoami().await {
            Ok(account) => emit(Event::Account(account.map(|a| a.account))),
            Err(e) => emit(Event::Error(e.to_string())),
        },

        Request::Login { account, password } => match client.login(&account, &password).await {
            Ok(token) => {
                let mut cfg = twentyfourbit_core::Config::load().unwrap_or_default();
                cfg.token = Some(token);
                match cfg.save() {
                    Ok(()) => emit(Event::Account(Some(account))),
                    Err(e) => emit(Event::Error(e.to_string())),
                }
            }
            Err(e) => emit(Event::Error(e.to_string())),
        },
    }
}

/// Copy a cached file to a user-visible location, creating the directory.
///
/// Copy rather than move: the cache still owns its entry and must keep working.
fn export_file(from: &std::path::Path, to: &std::path::Path) -> Result<(), String> {
    if let Some(parent) = to.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
    }
    std::fs::copy(from, to).map_err(|e| format!("{}: {e}", to.display()))?;
    Ok(())
}

/// Emit the cache contents as library rows, newest first.
pub fn list_library(cache: &Cache, emit: impl Fn(Event) + Send) {
    let mut rows = Vec::new();
    collect_files(cache.root(), &mut rows);

    rows.sort_by(|a, b| b.name.cmp(&a.name));

    let bytes = rows.iter().map(|r| r.bytes).sum();
    emit(Event::Library(rows));
    emit(Event::CacheStats {
        files: count_files(cache.root()),
        bytes,
    });
}

/// Emit the cache size after clearing it.
pub fn clear_cache(cache: &Cache, emit: impl Fn(Event) + Send) {
    match cache.clear() {
        Ok(()) => {
            emit(Event::Library(Vec::new()));
            emit(Event::CacheStats { files: 0, bytes: 0 });
        }
        Err(e) => emit(Event::Error(e.to_string())),
    }
}

fn collect_files(dir: &std::path::Path, out: &mut Vec<LibraryRow>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(meta) = entry.metadata() else { continue };
        if meta.is_dir() {
            collect_files(&path, out);
            continue;
        }
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        if ext == "part" || ext == "json" {
            continue;
        }
        out.push(LibraryRow {
            name: path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default(),
            path,
            bytes: meta.len(),
        });
    }
}

fn count_files(dir: &std::path::Path) -> usize {
    let mut rows = Vec::new();
    collect_files(dir, &mut rows);
    rows.len()
}

/// A handle to the running worker.
pub struct Backend {
    requests: Sender<Request>,
    events: Receiver<Event>,
}

impl Backend {
    /// Spawn the worker with its own tokio runtime.
    pub fn start(client: Client, cache: Cache) -> Self {
        let (req_tx, req_rx) = std::sync::mpsc::channel::<Request>();
        let (ev_tx, ev_rx) = std::sync::mpsc::channel::<Event>();

        std::thread::Builder::new()
            .name("24bit-gui-backend".into())
            .spawn(move || {
                let runtime = match tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                {
                    Ok(rt) => rt,
                    Err(e) => {
                        let _ = ev_tx.send(Event::Error(format!("no async runtime: {e}")));
                        return;
                    }
                };

                while let Ok(request) = req_rx.recv() {
                    let sender = ev_tx.clone();
                    runtime.block_on(handle(&client, &cache, request, move |event| {
                        let _ = sender.send(event);
                    }));
                }
            })
            .expect("cannot spawn backend thread");

        Self {
            requests: req_tx,
            events: ev_rx,
        }
    }

    /// Queue a request.
    pub fn send(&self, request: Request) {
        let _ = self.requests.send(request);
    }

    /// Drain pending events without blocking.
    pub fn poll(&self) -> Vec<Event> {
        self.events.try_iter().collect()
    }
}
