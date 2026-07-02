use crate::console::spinner;
use crate::{cprint_normal, cprint_verbose};
use bo4e_schemas::models::schema_meta::{Schema, Schemas};
use bo4e_schemas::models::version::Version;
use flate2::read::GzDecoder;
use http::StatusCode;
use reqwest::Client;
use reqwest::header::{ACCEPT, AUTHORIZATION, HeaderMap, HeaderValue};
use serde::Deserialize;
use std::io::Read;
use std::str::FromStr;
use std::sync::LazyLock;
use tar::Archive;

const GITHUB_API: &str = "https://api.github.com";
const OWNER: &str = "bo4e";
const REPO: &str = "BO4E-Schemas";
const USER_AGENT: &str = concat!("bo4e-cli/", env!("CARGO_PKG_VERSION"));
const GH_API_VERSION: &str = "2022-11-28";

// The `gh*_` arm intentionally has no upper length bound and accepts
// the full base64url alphabet plus `.`. GitHub now issues Actions
// installation tokens (`ghs_…`) as JWT-encoded blobs — three
// base64url segments joined by `.` — which the previous
// `[A-Za-z0-9_]` body charset rejected. Anchoring with `^…$` plus
// the character class still keeps the match tight; the real
// authority on token validity is GitHub itself.
static REGEX_GITHUB_TOKEN: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r"^(gh[pousr]_[A-Za-z0-9_.\-]{36,}|github_pat_[a-zA-Z0-9]{22}_[a-zA-Z0-9]{59}|v[0-9]\.[0-9a-f]{40})$").unwrap()
});
// Matches a schema path *relative to the repo root* (the tarball's
// top-level `<owner>-<repo>-<sha>/` prefix is stripped first). The
// `module` capture keeps sub-directories (`bo/Foo`, `enum/Bar`).
static REGEX_GITHUB_SRC_PATH: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"^src/bo4e_schemas/(?P<module>.*)\.json$").unwrap());

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

/// Subset of a GitHub Release we consume.
#[derive(Deserialize)]
struct Release {
    #[serde(default)]
    tag_name: String,
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

/// Download the whole `bo4e/BO4E-Schemas` repo at `version_tag` as a single
/// gzipped tarball (`GET .../tarball/{ref}`). One request instead of a tree
/// walk + one fetch per file, and — for anonymous users — one hit against the
/// 60/hour limit instead of ~200.
async fn download_schema_tarball(
    client: &Client,
    version_tag: &Version,
) -> Result<Vec<u8>, String> {
    let _spin = spinner::earth("Downloading schema archive");
    let url = format!("{GITHUB_API}/repos/{OWNER}/{REPO}/tarball/{version_tag}");
    let resp = client
        .get(url)
        // The tarball endpoint 302s to codeload; reqwest follows by default.
        .header(ACCEPT, "application/vnd.github+json")
        .send()
        .await
        .map_err(|e| format!("schema archive download request failed: {e}"))?;
    if !resp.status().is_success() {
        return Err(github_error(resp, "schema archive download").await);
    }
    let bytes = resp
        .bytes()
        .await
        .map_err(|e| format!("failed to read schema archive body: {e}"))?;
    Ok(bytes.to_vec())
}

/// Unpack a GitHub repo tarball in-process and build a `Schema` for every
/// `src/bo4e_schemas/**/*.json` entry. Pure CPU — no network, no temp files.
///
/// GitHub prefixes every entry with a `<owner>-<repo>-<sha>/` top-level
/// directory; that first path component is stripped before matching.
fn unpack_schemas(gz: &[u8]) -> Result<Vec<Schema>, String> {
    let decoder = GzDecoder::new(gz);
    let mut archive = Archive::new(decoder);
    let entries = archive
        .entries()
        .map_err(|e| format!("failed to read schema archive: {e}"))?;

    let mut schemas = Vec::new();
    for entry in entries {
        let mut entry = entry.map_err(|e| format!("corrupt entry in schema archive: {e}"))?;

        // tar stores '/'-separated paths as raw bytes; normalise defensively
        // and drop the top-level `<owner>-<repo>-<sha>/` component.
        let raw = entry.path_bytes();
        let full = String::from_utf8_lossy(&raw).replace('\\', "/");
        let Some((_, relative)) = full.split_once('/') else {
            continue;
        };

        let Some(path_match) = REGEX_GITHUB_SRC_PATH.captures(relative) else {
            continue;
        };
        let module = path_match.name("module").unwrap().as_str().to_string();

        let mut content = String::new();
        entry
            .read_to_string(&mut content)
            .map_err(|e| format!("failed to read schema {relative} from archive: {e}"))?;

        let mut schema = Schema::new(module.split('/').map(String::from).collect(), None)?;
        schema.load_schema(content);
        cprint_verbose!("Unpacked schema {}", relative);
        schemas.push(schema);
    }
    Ok(schemas)
}

/// Query the GitHub API of `bo4e/BO4E-Schemas` for a specific version.
/// Returns metadata of all BO4E schemas.
pub async fn get_schemas_from_github(
    version_tag: &Version,
    token: Option<&str>,
) -> Result<Schemas, String> {
    let client = build_client(token)?;
    let tarball = download_schema_tarball(&client, version_tag).await?;
    let schemas_vector = unpack_schemas(&tarball)?;
    cprint_normal!("Downloaded and unpacked {} schemas.", schemas_vector.len());
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
    use flate2::Compression;
    use flate2::write::GzEncoder;
    use std::io::Write;

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

    /// Build a gzipped tar mimicking GitHub's tarball layout (top-level
    /// `<owner>-<repo>-<sha>/` prefix on every entry).
    fn make_tarball(files: &[(&str, &str)]) -> Vec<u8> {
        let mut tar_bytes = Vec::new();
        {
            let mut builder = tar::Builder::new(&mut tar_bytes);
            for (path, body) in files {
                let mut header = tar::Header::new_gnu();
                header.set_size(body.len() as u64);
                header.set_mode(0o644);
                header.set_cksum();
                builder
                    .append_data(&mut header, path, body.as_bytes())
                    .unwrap();
            }
            builder.finish().unwrap();
        }
        let mut gz = Vec::new();
        let mut encoder = GzEncoder::new(&mut gz, Compression::default());
        encoder.write_all(&tar_bytes).unwrap();
        encoder.finish().unwrap();
        gz
    }

    /// `unpack_schemas` emits `cprint_verbose!` per file, which requires the
    /// global console to exist. Initialise it once (idempotent across tests).
    fn ensure_console() {
        use crate::console::console::{CONSOLE, Console, Level};
        let _ = CONSOLE.set(Console::new(Level::Normal));
    }

    #[test]
    fn unpack_selects_only_schema_jsons_and_strips_top_dir() {
        ensure_console();
        let gz = make_tarball(&[
            (
                "bo4e-BO4E-Schemas-abc1234/src/bo4e_schemas/bo/Angebot.json",
                r#"{"title":"Angebot"}"#,
            ),
            (
                "bo4e-BO4E-Schemas-abc1234/src/bo4e_schemas/enum/Typ.json",
                r#"{"title":"Typ"}"#,
            ),
            // Not under src/bo4e_schemas — must be ignored.
            ("bo4e-BO4E-Schemas-abc1234/README.md", "nope"),
            ("bo4e-BO4E-Schemas-abc1234/package.json", "{}"),
            (
                "bo4e-BO4E-Schemas-abc1234/src/bo4e_schemas/index.txt",
                "nope",
            ),
        ]);

        let schemas = unpack_schemas(&gz).unwrap();
        assert_eq!(
            schemas.len(),
            2,
            "only the two schema JSONs should be picked up"
        );

        // Top-level `<owner>-<repo>-<sha>/` prefix stripped; module sub-paths kept.
        let mut modules: Vec<Vec<String>> = schemas.iter().map(|s| s.module().to_vec()).collect();
        modules.sort();
        assert_eq!(
            modules,
            vec![
                vec!["bo".to_string(), "Angebot".to_string()],
                vec!["enum".to_string(), "Typ".to_string()],
            ]
        );
    }

    #[test]
    fn unpack_empty_archive_yields_no_schemas() {
        ensure_console();
        let gz = make_tarball(&[("bo4e-BO4E-Schemas-abc1234/README.md", "nothing here")]);
        assert!(unpack_schemas(&gz).unwrap().is_empty());
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
