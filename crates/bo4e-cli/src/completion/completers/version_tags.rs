use crate::completion::completers::cache::{Cache, CacheError, FetchResult};
use crate::models::cli::{Token, resolve_token_silent};
use chrono::{Duration, Utc};
use clap_complete::CompletionCandidate;
use std::path::PathBuf;

const TTL_SECS: i64 = 60;
const HARD_EXPIRY_HOURS: i64 = 24;

fn cache_path() -> PathBuf {
    dirs::cache_dir()
        .or_else(dirs::home_dir)
        .unwrap_or_else(|| PathBuf::from("."))
        .join("bo4e/versions.json")
}

/// Public entry point wired in via `ArgValueCandidates::new(...)`.
pub fn complete(prefix: &std::ffi::OsStr) -> Vec<CompletionCandidate> {
    let prefix = prefix.to_string_lossy().to_string();
    let token = resolve_token_silent(&extract_token_from_env());
    let runtime = match tokio::runtime::Runtime::new() {
        Ok(r) => r,
        Err(_) => return Vec::new(),
    };
    let cache = Cache::<Vec<String>>::new(
        cache_path(),
        Duration::seconds(TTL_SECS),
        Duration::hours(HARD_EXPIRY_HOURS),
    );
    let versions = cache.get_or_fetch(Utc::now(), |etag| {
        runtime.block_on(fetch(etag, token.as_deref()))
    });
    versions
        .unwrap_or_default()
        .into_iter()
        .filter(|v| v.starts_with(&prefix))
        .map(CompletionCandidate::new)
        .collect()
}

fn extract_token_from_env() -> Option<Token> {
    std::env::var("GITHUB_ACCESS_TOKEN")
        .ok()
        .and_then(|t| Token::new(t).ok())
}

async fn fetch(
    etag: Option<&str>,
    token: Option<&str>,
) -> Result<FetchResult<Vec<String>>, CacheError> {
    // Use reqwest directly — octocrab doesn't expose ETag headers ergonomically.
    let client = reqwest::Client::builder()
        .user_agent("bo4e-cli/version-completer")
        .timeout(std::time::Duration::from_millis(1500))
        .build()
        .map_err(|e| CacheError::Fetch(e.to_string()))?;

    let mut req = client.get("https://api.github.com/repos/bo4e/BO4E-Schemas/releases");
    if let Some(e) = etag {
        req = req.header("If-None-Match", e);
    }
    if let Some(t) = token {
        req = req.header("Authorization", format!("Bearer {t}"));
    }
    let resp = req
        .send()
        .await
        .map_err(|e| CacheError::Fetch(e.to_string()))?;
    let status = resp.status();
    let new_etag = resp
        .headers()
        .get("etag")
        .and_then(|h| h.to_str().ok())
        .map(String::from);

    if status.as_u16() == 304 {
        return Ok(FetchResult::NotModified);
    }
    if !status.is_success() {
        return Err(CacheError::Fetch(format!("HTTP {status}")));
    }
    let body: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| CacheError::Fetch(e.to_string()))?;
    let tags = body
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.get("tag_name").and_then(|t| t.as_str()).map(String::from))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    Ok(FetchResult::Replaced(tags, new_etag))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_token_returns_none_when_env_unset() {
        let prev = std::env::var("GITHUB_ACCESS_TOKEN").ok();
        // SAFETY: test suite is single-threaded for this env var
        unsafe {
            std::env::remove_var("GITHUB_ACCESS_TOKEN");
        }
        let r = extract_token_from_env();
        if let Some(v) = prev {
            unsafe {
                std::env::set_var("GITHUB_ACCESS_TOKEN", v);
            }
        }
        assert!(r.is_none());
    }

    // The fetch() function calls real GitHub; we don't unit-test that here.
    // The cache-fallback behaviour is tested in cache.rs.
}
