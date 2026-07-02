use crate::console::progress_bar::{
    abandon_progress_bar_with_error, finish_progress_bar, new_progress_bar,
};
use crate::console::spinner;
use crate::{cprint_normal, cprint_verbose};
use bo4e_schemas::models::schema_meta::{Schema, Schemas};
use bo4e_schemas::models::version::Version;
use http::StatusCode;
use lazy_static::lazy_static;
use reqwest::Client;
use reqwest::header::{ACCEPT, AUTHORIZATION, HeaderMap, HeaderValue};
use serde::Deserialize;
use std::pin::Pin;
use std::str::FromStr;
use tokio::task::JoinSet;

const GITHUB_API: &str = "https://api.github.com";
const OWNER: &str = "bo4e";
const REPO: &str = "BO4E-Schemas";
const USER_AGENT: &str = concat!("bo4e-cli/", env!("CARGO_PKG_VERSION"));
const GH_API_VERSION: &str = "2022-11-28";

lazy_static! {
    // The `gh*_` arm intentionally has no upper length bound and accepts
    // the full base64url alphabet plus `.`. GitHub now issues Actions
    // installation tokens (`ghs_…`) as JWT-encoded blobs — three
    // base64url segments joined by `.` — which the previous
    // `[A-Za-z0-9_]` body charset rejected. Anchoring with `^…$` plus
    // the character class still keeps the match tight; the real
    // authority on token validity is GitHub itself.
    static ref REGEX_GITHUB_TOKEN: regex::Regex = regex::Regex::new(r"^(gh[pousr]_[A-Za-z0-9_.\-]{36,}|github_pat_[a-zA-Z0-9]{22}_[a-zA-Z0-9]{59}|v[0-9]\.[0-9a-f]{40})$").unwrap();
    static ref REGEX_GITHUB_SRC_PATH: regex::Regex = regex::Regex::new(r"^src/bo4e_schemas/(?P<module>.*)\.json$").unwrap();
}

pub fn is_valid_github_token(token: &str) -> bool {
    REGEX_GITHUB_TOKEN.is_match(token)
}

pub fn get_token_from_github_cli() -> Option<String> {
    let output = match std::process::Command::new("gh")
        .arg("auth")
        .arg("token")
        .output()
    {
        Ok(o) => o,
        Err(e) => {
            cprint_verbose!("`gh auth token` not invokable ({e}); skipping CLI token lookup");
            return None;
        }
    };
    if !output.status.success() {
        cprint_verbose!(
            "`gh auth token` exited with {}; not logged in to gh? skipping",
            output.status
        );
        return None;
    }
    let token_str = String::from_utf8_lossy(&output.stdout);
    let token_str = token_str.trim();
    if token_str.is_empty() {
        cprint_verbose!("`gh auth token` returned empty output; skipping");
        return None;
    }
    if !is_valid_github_token(token_str) {
        cprint_verbose!(
            "`gh auth token` returned a token whose format isn't recognised by bo4e-cli's \
             validator; ignoring it. Pass --token explicitly or set GITHUB_ACCESS_TOKEN."
        );
        return None;
    }
    cprint_normal!("Retrieved access token from GitHub CLI command `gh auth token`.");
    Some(token_str.to_string())
}

/// GitHub content-listing entry (`GET /repos/{o}/{r}/contents/{path}`).
#[derive(Deserialize)]
struct ContentItem {
    path: String,
    #[serde(rename = "type")]
    kind: String,
}

/// Subset of a GitHub Release we consume.
#[derive(Deserialize)]
struct Release {
    #[serde(default)]
    tag_name: String,
    #[serde(default)]
    target_commitish: String,
}

/// GitHub error bodies are `{ "message": "...", "documentation_url": "..." }`.
#[derive(Deserialize)]
struct GithubErrorBody {
    message: Option<String>,
}

/// Build a reqwest client carrying the headers GitHub's REST API expects:
/// a `User-Agent` (mandatory), the pinned API version, and — if a token is
/// supplied — a bearer `Authorization` header marked sensitive so it is not
/// logged or forwarded on redirect.
fn build_client(token: Option<&str>) -> Result<Client, String> {
    let mut headers = HeaderMap::new();
    headers.insert(
        "X-GitHub-Api-Version",
        HeaderValue::from_static(GH_API_VERSION),
    );
    if let Some(token) = token {
        let mut auth = HeaderValue::from_str(&format!("Bearer {token}"))
            .map_err(|e| format!("invalid GitHub token for Authorization header: {e}"))?;
        auth.set_sensitive(true);
        headers.insert(AUTHORIZATION, auth);
    }
    Client::builder()
        .user_agent(USER_AGENT)
        .default_headers(headers)
        .build()
        .map_err(|e| format!("failed to build HTTP client: {e}"))
}

fn contents_url(path: &str, commitish: &str) -> String {
    format!("{GITHUB_API}/repos/{OWNER}/{REPO}/contents/{path}?ref={commitish}")
}

/// Turn a non-success GitHub response into an actionable end-user message.
///
/// FORBIDDEN / TOO_MANY_REQUESTS almost always mean unauthenticated
/// rate-limiting; surface a hint rather than the bare status.
async fn github_error(resp: reqwest::Response, context: &str) -> String {
    let status = resp.status();
    let message = resp
        .json::<GithubErrorBody>()
        .await
        .ok()
        .and_then(|b| b.message)
        .unwrap_or_else(|| status.to_string());
    if status == StatusCode::FORBIDDEN || status == StatusCode::TOO_MANY_REQUESTS {
        return format!(
            "GitHub rate-limited the {context} request ({message}). \
             Authenticate to lift the limit: pass --token, set GITHUB_ACCESS_TOKEN, \
             or run `gh auth login`."
        );
    }
    if status == StatusCode::NOT_FOUND {
        return format!("GitHub returned 404 for the {context} request: {message}");
    }
    format!("GitHub returned {status} for the {context} request: {message}")
}

async fn list_dir(
    client: &Client,
    commitish: &str,
    dir_path: &str,
) -> Result<Vec<ContentItem>, String> {
    let resp = client
        .get(contents_url(dir_path, commitish))
        .header(ACCEPT, "application/vnd.github+json")
        .send()
        .await
        .map_err(|e| format!("schema directory listing request failed: {e}"))?;
    if !resp.status().is_success() {
        return Err(github_error(resp, "schema directory listing").await);
    }
    resp.json::<Vec<ContentItem>>()
        .await
        .map_err(|e| format!("failed to parse schema directory listing: {e}"))
}

/// Fetch a single file's raw bytes via `Accept: application/vnd.github.raw`,
/// which returns the content directly (no base64 wrapper to decode).
async fn fetch_file_raw(
    client: &Client,
    commitish: &str,
    file_path: &str,
) -> Result<String, String> {
    let resp = client
        .get(contents_url(file_path, commitish))
        .header(ACCEPT, "application/vnd.github.raw")
        .send()
        .await
        .map_err(|e| format!("schema file fetch request failed: {e}"))?;
    if !resp.status().is_success() {
        return Err(github_error(resp, "schema file fetch").await);
    }
    resp.text()
        .await
        .map_err(|e| format!("failed to read schema file content: {e}"))
}

type AsyncInvokeLater<T> = Pin<Box<dyn Future<Output = T>>>;

async fn _get_schemas_from_github_recursive(
    client: Client,
    target_commitish: String,
    dir_path: String,
) -> Result<Vec<AsyncInvokeLater<Result<Schema, String>>>, String> {
    let items = list_dir(&client, &target_commitish, &dir_path).await?;

    let mut futures: Vec<AsyncInvokeLater<Result<Schema, String>>> = Vec::new();

    for item in items {
        match item.kind.as_str() {
            "file" => {
                if let Some(path_match) = REGEX_GITHUB_SRC_PATH.captures(&item.path) {
                    let client = client.clone();
                    let target_commitish = target_commitish.clone();
                    let file_path = item.path.clone();
                    let path_slice = path_match.name("module").unwrap().as_str().to_string();

                    futures.push(Box::pin(async move {
                        let file_content =
                            fetch_file_raw(&client, &target_commitish, &file_path).await?;
                        cprint_verbose!("Fetched schema {}", file_path);
                        let mut schema =
                            Schema::new(path_slice.split('/').map(String::from).collect(), None)?;
                        schema.load_schema(file_content);
                        Ok(schema)
                    }));
                }
            }
            "dir" => {
                futures.append(
                    &mut Box::pin(_get_schemas_from_github_recursive(
                        client.clone(),
                        target_commitish.clone(),
                        item.path.clone(),
                    ))
                    .await?,
                );
            }
            _ => {
                // Ignore other types (e.g., symlinks, submodules)
            }
        }
    }
    Ok(futures)
}

async fn _execute_futures_with_progress_bar<T: 'static>(
    futures: Vec<AsyncInvokeLater<T>>,
) -> Result<Vec<T>, String> {
    let total = futures.len();
    let start_message = "Downloading schemas...";
    let finish_message = "Downloaded schemas.   ";
    let visible = crate::console::console::CONSOLE
        .get()
        .map(|c| c.would_emit(crate::console::console::Level::Normal))
        .unwrap_or(true);
    let pb = visible.then(|| new_progress_bar(total as u64, Some(start_message.to_string())));

    let mut join_set: JoinSet<T> = JoinSet::new();
    for future in futures {
        join_set.spawn_local(future);
    }

    let mut output = Ok(Vec::new());
    while let Some(res) = join_set.join_next().await {
        if output.is_err() {
            // Entering here means all tasks have been aborted due to a panic or error.
            continue;
        }
        match res {
            Ok(value) => output.as_mut().unwrap().push(value),
            Err(err) if err.is_panic() => {
                output = Err(format!("Panic occurred: {:?}", err));
                join_set.abort_all();
            }
            Err(err) => {
                output = Err(format!("Task joining failed: {:?}", err));
                join_set.abort_all();
            }
        }
        if let Some(ref pb) = pb {
            pb.inc(1);
        }
    }

    if let Some(ref pb) = pb {
        if let Err(ref e) = output {
            abandon_progress_bar_with_error(pb, format!("Error: {}", e));
        } else {
            finish_progress_bar(pb, Some(finish_message.to_string()));
        }
    }

    output
}

async fn get_target_commitish_from_tag(
    client: &Client,
    version_tag: &Version,
) -> Result<String, String> {
    let _spin = spinner::earth("Querying GitHub tree");
    let url = format!("{GITHUB_API}/repos/{OWNER}/{REPO}/releases/tags/{version_tag}");
    let resp = client
        .get(url)
        .header(ACCEPT, "application/vnd.github+json")
        .send()
        .await
        .map_err(|e| format!("release lookup request failed: {e}"))?;
    if !resp.status().is_success() {
        return Err(github_error(resp, "release lookup").await);
    }
    let release: Release = resp
        .json()
        .await
        .map_err(|e| format!("failed to parse release lookup response: {e}"))?;
    cprint_verbose!(
        "Resolved tag {} → commitish {}",
        version_tag,
        release.target_commitish
    );
    Ok(release.target_commitish)
}

/// Query the GitHub API of `bo4e/BO4E-Schemas` for a specific version.
/// Returns metadata of all BO4E schemas.
pub async fn get_schemas_from_github(
    version_tag: &Version,
    token: Option<&str>,
) -> Result<Schemas, String> {
    let client = build_client(token)?;
    let target_commitish = get_target_commitish_from_tag(&client, version_tag).await?;

    // Scoped so the spinner drops before the download progress bar takes over.
    let schema_downloads = {
        let _spin = spinner::earth("Querying GitHub tree");
        _get_schemas_from_github_recursive(client, target_commitish, "src/bo4e_schemas".to_string())
            .await?
    };
    cprint_normal!(
        "Queried GitHub tree. Found {} schemas.",
        schema_downloads.len()
    );
    let local_set = tokio::task::LocalSet::new();
    let schemas_vector = local_set
        .run_until(_execute_futures_with_progress_bar(schema_downloads))
        .await?
        .into_iter()
        .collect::<Result<Vec<Schema>, String>>()?;
    let schemas = Schemas::try_from((schemas_vector, version_tag.into()))?;

    Ok(schemas)
}

pub async fn resolve_latest_version(token: Option<&str>) -> Result<Version, String> {
    let version = {
        let _spin = spinner::earth("Querying GitHub for latest version");
        let client = build_client(token)?;
        let url = format!("{GITHUB_API}/repos/{OWNER}/{REPO}/releases/latest");
        let resp = client
            .get(url)
            .header(ACCEPT, "application/vnd.github+json")
            .send()
            .await
            .map_err(|e| format!("latest-release lookup request failed: {e}"))?;
        if !resp.status().is_success() {
            return Err(github_error(resp, "latest-release lookup").await);
        }
        let release: Release = resp
            .json()
            .await
            .map_err(|e| format!("failed to parse latest-release response: {e}"))?;
        Version::from_str(&release.tag_name)?
    };
    cprint_normal!("Resolved latest release to {}", version);
    Ok(version)
}

/// Check if a GitHub *Release* exists in `bo4e/BO4E-Schemas` for the given version.
///
/// Note: a Release is more than a pushed tag. A tag without an associated Release
/// returns 404 from the `releases/tags/{tag}` endpoint and is treated as `Ok(false)`.
pub async fn release_exists(version: &Version, token: Option<&str>) -> Result<bool, String> {
    let client = build_client(token)?;
    let url = format!("{GITHUB_API}/repos/{OWNER}/{REPO}/releases/tags/{version}");
    let resp = client
        .get(url)
        .header(ACCEPT, "application/vnd.github+json")
        .send()
        .await
        .map_err(|e| format!("release validation request failed: {e}"))?;
    match resp.status() {
        s if s.is_success() => Ok(true),
        StatusCode::NOT_FOUND => Ok(false),
        _ => Err(github_error(resp, "release validation").await),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Compile-time check that `release_exists` has the expected signature.
    /// We can't make a live network call in unit tests.
    #[test]
    fn test_release_exists_signature() {
        fn _assert_signature(
            v: &Version,
        ) -> impl std::future::Future<Output = Result<bool, String>> + '_ {
            release_exists(v, None)
        }
        let _ = _assert_signature; // silence unused
    }

    #[test]
    fn classic_pat_is_valid() {
        // Classic PATs are 40 chars total: `ghp_` + 36 body chars.
        let token = format!("ghp_{}", "a".repeat(36));
        assert!(is_valid_github_token(&token));
    }

    #[test]
    fn fine_grained_pat_is_valid() {
        // `github_pat_` + 22 chars + `_` + 59 chars.
        let token = format!("github_pat_{}_{}", "a".repeat(22), "b".repeat(59));
        assert!(is_valid_github_token(&token));
    }

    #[test]
    fn long_actions_installation_token_is_valid() {
        // Regression: GitHub Actions now issues `ghs_…` installation tokens
        // well over 400 chars. The previous {36,251} cap rejected them, which
        // broke `bo4e pull` when invoked from a GitHub Actions workflow with
        // `GITHUB_TOKEN`.
        let token = format!("ghs_{}", "A".repeat(422));
        assert_eq!(token.len(), 426);
        assert!(is_valid_github_token(&token));
    }

    #[test]
    fn jwt_style_installation_token_is_valid() {
        // Regression: GitHub Actions installation tokens are now JWT-encoded
        // (`header.payload.signature`, three base64url segments joined by
        // `.`). Confirmed in BO4E-Python's docs CI on 2026-06-05 by sorting
        // the body chars and finding two `.` near the start. The prior
        // `[A-Za-z0-9_]` body charset rejected them.
        let header = "A".repeat(140);
        let payload = "B".repeat(140);
        let signature = "C".repeat(140);
        let token = format!("ghs_{header}.{payload}.{signature}");
        assert_eq!(token.len(), 426);
        assert!(is_valid_github_token(&token));
    }

    #[test]
    fn base64url_dash_is_valid_in_body() {
        // base64url uses `-` where base64 uses `+`. JWT signatures and
        // payloads can contain `-`, so the body charset must accept it.
        let token = format!("ghs_{}", "a-b_".repeat(40));
        assert!(is_valid_github_token(&token));
    }

    #[test]
    fn garbage_is_rejected() {
        assert!(!is_valid_github_token(""));
        assert!(!is_valid_github_token("not-a-token"));
        assert!(!is_valid_github_token("ghp_short"));
        // Wrong prefix letter (only p/o/u/s/r are accepted after `gh`).
        assert!(!is_valid_github_token(&format!("ghx_{}", "a".repeat(36))));
    }
}
