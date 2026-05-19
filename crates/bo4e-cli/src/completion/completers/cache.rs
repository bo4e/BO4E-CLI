// crates/bo4e-cli/src/completion/completers/cache.rs
use chrono::{DateTime, Duration, Utc};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheEntry<T> {
    pub etag: Option<String>,
    pub fetched_at: DateTime<Utc>,
    pub body: T,
}

#[derive(Debug)]
pub enum FetchResult<T> {
    /// Server returned new content; replace cache.
    Replaced(T, Option<String>),
    /// Server returned 304; keep existing body, refresh timestamp.
    NotModified,
}

#[derive(Debug, thiserror::Error)]
pub enum CacheError {
    #[error("io: {0}")]
    Io(#[from] io::Error),
    #[error("serde: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("fetch failed: {0}")]
    Fetch(String),
}

pub struct Cache<T> {
    pub path: PathBuf,
    pub ttl: Duration,
    /// If cached entry is older than this AND fetch fails, return None instead
    /// of stale.
    pub hard_expiry: Duration,
    _phantom: std::marker::PhantomData<T>,
}

impl<T: Serialize + DeserializeOwned + Clone> Cache<T> {
    pub fn new(path: PathBuf, ttl: Duration, hard_expiry: Duration) -> Self {
        Self {
            path,
            ttl,
            hard_expiry,
            _phantom: std::marker::PhantomData,
        }
    }

    /// Read the cached entry from disk, or None on any error.
    pub fn read(&self) -> Option<CacheEntry<T>> {
        let bytes = fs::read(&self.path).ok()?;
        serde_json::from_slice(&bytes).ok()
    }

    pub fn write(&self, entry: &CacheEntry<T>) -> Result<(), CacheError> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        let bytes = serde_json::to_vec(entry)?;
        fs::write(&self.path, bytes)?;
        Ok(())
    }

    /// Resolve the cache for use. `fetch(etag)` should perform a conditional
    /// GET and return either `Replaced(body, new_etag)` on 200 or
    /// `NotModified` on 304.
    pub fn get_or_fetch<F>(&self, now: DateTime<Utc>, fetch: F) -> Option<T>
    where
        F: FnOnce(Option<&str>) -> Result<FetchResult<T>, CacheError>,
    {
        let cached = self.read();
        if let Some(entry) = &cached
            && now - entry.fetched_at < self.ttl
        {
            return Some(entry.body.clone());
        }
        let etag = cached.as_ref().and_then(|e| e.etag.clone());
        match fetch(etag.as_deref()) {
            Ok(FetchResult::Replaced(body, new_etag)) => {
                let entry = CacheEntry {
                    etag: new_etag,
                    fetched_at: now,
                    body: body.clone(),
                };
                let _ = self.write(&entry);
                Some(body)
            }
            Ok(FetchResult::NotModified) => {
                if let Some(entry) = cached {
                    let refreshed = CacheEntry {
                        etag: entry.etag.clone(),
                        fetched_at: now,
                        body: entry.body.clone(),
                    };
                    let _ = self.write(&refreshed);
                    Some(entry.body)
                } else {
                    None
                }
            }
            Err(_) => {
                if let Some(entry) = cached
                    && now - entry.fetched_at < self.hard_expiry
                {
                    return Some(entry.body);
                }
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn make_cache(td: &TempDir, ttl_s: i64) -> Cache<Vec<String>> {
        Cache::new(
            td.path().join("c.json"),
            Duration::seconds(ttl_s),
            Duration::hours(24),
        )
    }

    #[test]
    fn fresh_within_ttl_serves_cached_without_fetch() {
        let td = TempDir::new().unwrap();
        let c = make_cache(&td, 60);
        let now = Utc::now();
        c.write(&CacheEntry {
            etag: Some("E1".into()),
            fetched_at: now - Duration::seconds(10),
            body: vec!["v1".into()],
        })
        .unwrap();
        let v = c
            .get_or_fetch(now, |_etag| panic!("should not fetch"))
            .unwrap();
        assert_eq!(v, vec!["v1".to_string()]);
    }

    #[test]
    fn expired_with_etag_sends_conditional_and_keeps_cached_on_304() {
        let td = TempDir::new().unwrap();
        let c = make_cache(&td, 60);
        let now = Utc::now();
        c.write(&CacheEntry {
            etag: Some("E1".into()),
            fetched_at: now - Duration::seconds(120),
            body: vec!["v1".into()],
        })
        .unwrap();
        let v = c
            .get_or_fetch(now, |etag| {
                assert_eq!(etag, Some("E1"));
                Ok(FetchResult::NotModified)
            })
            .unwrap();
        assert_eq!(v, vec!["v1".to_string()]);
        let entry = c.read().unwrap();
        assert_eq!(
            entry.fetched_at, now,
            "304 path should refresh fetched_at to now"
        );
    }

    #[test]
    fn expired_with_200_replaces_cache() {
        let td = TempDir::new().unwrap();
        let c = make_cache(&td, 60);
        let now = Utc::now();
        c.write(&CacheEntry {
            etag: Some("E1".into()),
            fetched_at: now - Duration::seconds(120),
            body: vec!["v1".into()],
        })
        .unwrap();
        let v = c
            .get_or_fetch(now, |_etag| {
                Ok(FetchResult::Replaced(vec!["v2".into()], Some("E2".into())))
            })
            .unwrap();
        assert_eq!(v, vec!["v2".to_string()]);
        let entry = c.read().unwrap();
        assert_eq!(entry.etag.as_deref(), Some("E2"));
        assert_eq!(entry.body, vec!["v2".to_string()]);
    }

    #[test]
    fn offline_within_hard_expiry_returns_stale() {
        let td = TempDir::new().unwrap();
        let c = make_cache(&td, 60);
        let now = Utc::now();
        c.write(&CacheEntry {
            etag: Some("E1".into()),
            fetched_at: now - Duration::hours(2),
            body: vec!["stale".into()],
        })
        .unwrap();
        let v = c
            .get_or_fetch(now, |_| Err(CacheError::Fetch("offline".into())))
            .unwrap();
        assert_eq!(v, vec!["stale".to_string()]);
    }

    #[test]
    fn offline_past_hard_expiry_returns_none() {
        let td = TempDir::new().unwrap();
        let c = make_cache(&td, 60);
        let now = Utc::now();
        c.write(&CacheEntry {
            etag: Some("E1".into()),
            fetched_at: now - Duration::hours(48),
            body: vec!["stale".into()],
        })
        .unwrap();
        let v = c.get_or_fetch(now, |_| Err(CacheError::Fetch("offline".into())));
        assert!(v.is_none());
    }

    #[test]
    fn cold_cache_with_200_writes() {
        let td = TempDir::new().unwrap();
        let c = make_cache(&td, 60);
        let now = Utc::now();
        let v = c
            .get_or_fetch(now, |etag| {
                assert!(etag.is_none());
                Ok(FetchResult::Replaced(vec!["v1".into()], Some("E1".into())))
            })
            .unwrap();
        assert_eq!(v, vec!["v1".to_string()]);
        assert!(c.read().is_some());
    }
}
