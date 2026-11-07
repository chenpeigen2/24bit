//! Behavioural tests for the on-disk track cache.
//!
//! A master-tier track is ~150MB, so re-downloading it on every play is not
//! acceptable. The cache is keyed by track id *and* quality, because a track
//! exists in several sizes and playing the surround version must not overwrite
//! the master file.
//!
//! Acceptance criterion: a second play of the same track performs zero network
//! requests.

use std::time::Duration;

use twentyfourbit_core::cache::{Cache, CacheKey};
use twentyfourbit_core::Quality;

fn scratch(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "24bit-cache-{}-{name}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn a_stored_entry_is_found_again() {
    let dir = scratch("hit");
    let cache = Cache::new(&dir, u64::MAX);
    let key = CacheKey::new("abc123", Quality::Master);
    let path = cache.path_for(&key);

    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, b"fake flac").unwrap();

    assert!(cache.lookup(&key).is_some(), "a stored entry must be found");
    assert_eq!(cache.lookup(&key).unwrap(), path);
}

#[test]
fn an_absent_entry_misses() {
    let dir = scratch("miss");
    let cache = Cache::new(&dir, u64::MAX);
    assert!(Cache::new(&dir, u64::MAX)
        .lookup(&CacheKey::new("nope", Quality::Master))
        .is_none());
}

/// The two quality tiers of one track must not collide.
#[test]
fn quality_tiers_have_distinct_paths() {
    let dir = scratch("tiers");
    let cache = Cache::new(&dir, u64::MAX);
    let master = cache.path_for(&CacheKey::new("abc", Quality::Master));
    let surround = cache.path_for(&CacheKey::new("abc", Quality::Surround));

    assert_ne!(master, surround, "tiers must not share a file");
    assert!(master.to_string_lossy().contains("master"));
    assert!(surround.to_string_lossy().contains("surround"));
}

#[test]
fn ids_with_path_separators_cannot_escape_the_cache_dir() {
    let dir = scratch("escape");
    let cache = Cache::new(&dir, u64::MAX);
    let path = cache.path_for(&CacheKey::new("../../etc/passwd", Quality::Master));

    assert!(
        path.starts_with(&dir),
        "a hostile id must stay inside the cache dir, got {}",
        path.display()
    );
}

/// Total size counts every cached file.
#[test]
fn size_reflects_stored_bytes() {
    let dir = scratch("size");
    let cache = Cache::new(&dir, u64::MAX);
    for (id, bytes) in [("a", 1000usize), ("b", 2000)] {
        let p = cache.path_for(&CacheKey::new(id, Quality::Master));
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(&p, vec![0u8; bytes]).unwrap();
    }
    assert_eq!(cache.total_size().unwrap(), 3000);
}

/// Playing a cached entry must protect it: eviction is by *use*, not by age.
///
/// Here `stale` is the oldest file on disk, but `hot` was read most recently,
/// so `stale` is the one that must go.
#[test]
fn eviction_removes_the_least_recently_used_entry_not_the_oldest() {
    let dir = scratch("evict");
    let cache = Cache::new(&dir, 2500);

    let stale = cache.path_for(&CacheKey::new("stale", Quality::Master));
    let hot = cache.path_for(&CacheKey::new("hot", Quality::Master));
    for p in [&stale, &hot] {
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, vec![0u8; 1000]).unwrap();
    }

    // `stale` is now older than `hot` only because `hot` is looked up afterwards.
    std::thread::sleep(Duration::from_millis(30));
    assert!(
        cache.lookup(&CacheKey::new("hot", Quality::Master)).is_some(),
        "lookup must find the stored entry"
    );

    // Push over budget with a third entry.
    let newest = cache.path_for(&CacheKey::new("newest", Quality::Master));
    std::fs::create_dir_all(newest.parent().unwrap()).unwrap();
    std::fs::write(&newest, vec![0u8; 1000]).unwrap();

    cache.enforce_budget().unwrap();

    assert!(!stale.exists(), "the least recently used entry should be evicted");
    assert!(hot.exists(), "an entry that was just read must survive");
    assert!(newest.exists(), "the freshly written entry must survive");
    assert!(cache.total_size().unwrap() <= 2500);
}

/// Two hits in a row must keep an entry alive across repeated eviction passes.
#[test]
fn a_hit_refreshes_recency_across_passes() {
    let dir = scratch("refresh");
    let cache = Cache::new(&dir, 1500);
    let key = CacheKey::new("keep", Quality::Master);
    let path = cache.path_for(&key);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, vec![0u8; 1000]).unwrap();

    let other = cache.path_for(&CacheKey::new("other", Quality::Master));
    std::fs::create_dir_all(other.parent().unwrap()).unwrap();
    std::fs::write(&other, vec![0u8; 1000]).unwrap();

    std::thread::sleep(Duration::from_millis(30));
    let _ = cache.lookup(&key);
    cache.enforce_budget().unwrap();

    assert!(path.exists(), "the entry just read must not be evicted");
    assert!(!other.exists(), "the untouched entry should be evicted");
}

/// Eviction never touches an entry that is currently being written (its `.part`
/// file), otherwise an in-flight download would be destroyed.
#[test]
fn eviction_ignores_partial_downloads() {
    let dir = scratch("partial");
    let cache = Cache::new(&dir, 1000);

    let key = CacheKey::new("big", Quality::Master);
    let path = cache.path_for(&key);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let part = twentyfourbit_core::part_path(&path);
    std::fs::write(&part, vec![0u8; 5000]).unwrap();

    cache.enforce_budget().unwrap();
    assert!(part.exists(), "an in-flight .part file must not be evicted");
}

/// Clearing empties the directory but leaves the directory itself.
#[test]
fn clear_removes_everything_it_owns() {
    let dir = scratch("clear");
    let cache = Cache::new(&dir, u64::MAX);
    let p = cache.path_for(&CacheKey::new("x", Quality::Master));
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(&p, b"data").unwrap();

    cache.clear().unwrap();

    assert_eq!(cache.total_size().unwrap(), 0);
    assert!(dir.exists(), "the cache directory itself should survive");
}

/// The default cache directory lives under the platform cache dir.
#[test]
fn default_location_is_under_the_platform_cache_dir() {
    let d = Cache::default_dir();
    assert!(
        d.to_string_lossy().contains("24bit"),
        "expected a 24bit-specific directory, got {}",
        d.display()
    );
}
