//! A bounded on-disk cache for downloaded tracks.
//!
//! A master-tier file is ~150MB, so treating every play as a fresh download is
//! not acceptable. Entries are keyed by track id *and* quality tier: the same
//! song exists in several sizes, and caching the surround version must not
//! destroy the master file.
//!
//! Sizing is enforced by least-recently-used eviction. Recency is the file's
//! modification time, which [`Cache::lookup`] bumps on every hit: access times
//! are unreliable (many filesystems mount `noatime`), and a track the user just
//! played must not be the first thing evicted.
//!
//! In-flight `.part` files are never evicted — destroying a half-finished
//! 150MB download to make room would be worse than exceeding the budget.

use std::path::{Path, PathBuf};

use crate::model::Quality;

/// Identifies one cached artefact.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CacheKey {
    /// Track id from the search results.
    pub id: String,
    /// Which quality tier this file holds.
    pub quality: Quality,
}

impl CacheKey {
    /// Build a key.
    pub fn new(id: impl Into<String>, quality: Quality) -> Self {
        Self {
            id: id.into(),
            quality,
        }
    }
}

/// A size-bounded directory of cached tracks.
#[derive(Debug, Clone)]
pub struct Cache {
    root: PathBuf,
    budget: u64,
}

impl Cache {
    /// The default cache directory (`~/.cache/24bit` on Linux and macOS).
    pub fn default_dir() -> PathBuf {
        dirs::cache_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("24bit")
    }

    /// Open (or lazily create) a cache at `root` with a byte budget.
    pub fn new(root: impl Into<PathBuf>, budget: u64) -> Self {
        Self {
            root: root.into(),
            budget,
        }
    }

    /// Open the default cache with the default budget.
    pub fn default_cache() -> Self {
        Self::new(Self::default_dir(), 5 * 1024 * 1024 * 1024)
    }

    /// Root directory.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Byte budget.
    pub fn budget(&self) -> u64 {
        self.budget
    }

    /// Where the file for `key` would live.
    ///
    /// The id is sanitised so that a hostile or malformed id cannot escape the
    /// cache directory.
    pub fn path_for(&self, key: &CacheKey) -> PathBuf {
        let safe: String = key
            .id
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
            .collect();
        self.root
            .join(tier_dir(key.quality))
            .join(format!("{safe}.{}", "flac"))
    }

    /// Where the metadata sidecar for `key` lives.
    ///
    /// Storing the track's title/artist next to the audio means a cache hit
    /// needs no network at all — not even for the labels in a library view.
    pub fn meta_path(&self, key: &CacheKey) -> PathBuf {
        self.path_for(key).with_extension("json")
    }

    /// Read the stored metadata sidecar, if present and parseable.
    pub fn read_meta<T: serde::de::DeserializeOwned>(&self, key: &CacheKey) -> Option<T> {
        let text = std::fs::read_to_string(self.meta_path(key)).ok()?;
        serde_json::from_str(&text).ok()
    }

    /// Write the metadata sidecar for `key`.
    pub fn write_meta<T: serde::Serialize>(
        &self,
        key: &CacheKey,
        value: &T,
    ) -> std::io::Result<()> {
        let path = self.meta_path(key);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let text = serde_json::to_string(value)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        std::fs::write(path, text)
    }

    /// Return the cached path for `key` if it exists and is non-empty.
    ///
    /// A zero-length file is treated as absent so that a crash mid-write cannot
    /// produce a permanently "cached" empty track.
    pub fn lookup(&self, key: &CacheKey) -> Option<PathBuf> {
        let path = self.path_for(key);
        let meta = std::fs::metadata(&path).ok()?;
        if meta.len() == 0 {
            return None;
        }
        // Mark as recently used so eviction protects it.
        if let Ok(file) = std::fs::OpenOptions::new().append(true).open(&path) {
            let _ = file.set_modified(std::time::SystemTime::now());
        }
        Some(path)
    }

    /// Total bytes held (including in-flight `.part` files).
    pub fn total_size(&self) -> std::io::Result<u64> {
        let mut total = 0;
        if !self.root.exists() {
            return Ok(0);
        }
        let mut stack = vec![self.root.clone()];
        while let Some(dir) = stack.pop() {
            let Ok(entries) = std::fs::read_dir(&dir) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                match entry.metadata() {
                    Ok(m) if m.is_dir() => stack.push(path),
                    Ok(m) => total += m.len(),
                    Err(_) => continue,
                }
            }
        }
        Ok(total)
    }

    /// Evict least-recently-used entries until the cache fits its budget.
    pub fn enforce_budget(&self) -> std::io::Result<()> {
        let mut total = self.total_size()?;
        if total <= self.budget {
            return Ok(());
        }

        let mut candidates = self.completed_entries()?;
        // Oldest access first.
        candidates.sort_by_key(|(_, used)| *used);

        for (path, _) in candidates {
            if total <= self.budget {
                break;
            }
            let Ok(meta) = std::fs::metadata(&path) else {
                continue;
            };
            let len = meta.len();
            if std::fs::remove_file(&path).is_ok() {
                // Take the metadata sidecar with it, or the library view would
                // show rows whose audio is gone.
                let _ = std::fs::remove_file(path.with_extension("json"));
                total = total.saturating_sub(len);
                tracing::debug!(path = %path.display(), "evicted cached track");
            }
        }
        Ok(())
    }

    /// Delete every cached file, keeping the directory itself.
    pub fn clear(&self) -> std::io::Result<()> {
        if !self.root.exists() {
            return Ok(());
        }
        for entry in std::fs::read_dir(&self.root)?.flatten() {
            let path = entry.path();
            if entry.metadata().map(|m| m.is_dir()).unwrap_or(false) {
                let _ = std::fs::remove_dir_all(&path);
            } else {
                let _ = std::fs::remove_file(&path);
            }
        }
        Ok(())
    }

    /// Finished entries with their last-access time, for eviction ordering.
    fn completed_entries(&self) -> std::io::Result<Vec<(PathBuf, std::time::SystemTime)>> {
        let mut out = Vec::new();
        if !self.root.exists() {
            return Ok(out);
        }
        let mut stack = vec![self.root.clone()];
        while let Some(dir) = stack.pop() {
            let Ok(entries) = std::fs::read_dir(&dir) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                let Ok(meta) = entry.metadata() else { continue };
                if meta.is_dir() {
                    stack.push(path);
                    continue;
                }
                // Never evict a download that is still being written.
                if path.extension().map(|e| e == "part").unwrap_or(false) {
                    continue;
                }
                let used = meta.modified().unwrap_or(std::time::UNIX_EPOCH);
                out.push((path, used));
            }
        }
        Ok(out)
    }
}

fn tier_dir(quality: Quality) -> &'static str {
    match quality {
        Quality::Master => "master",
        Quality::Surround => "surround",
    }
}
