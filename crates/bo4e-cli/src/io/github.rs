use crate::console::progress_bar::{
    abandon_progress_bar_with_error, finish_progress_bar, new_progress_bar,
};
use bo4e_schemas::models::schema_meta::{Schema, Schemas};
use bo4e_schemas::models::version::Version;
use lazy_static::lazy_static;
use octocrab::repos::RepoHandler;
use std::pin::Pin;
use std::str::FromStr;
use tokio::task::JoinSet;

lazy_static! {
    static ref REGEX_GITHUB_TOKEN: regex::Regex = regex::Regex::new(r"^(gh[pousr]_[A-Za-z0-9_]{36,251}|github_pat_[a-zA-Z0-9]{22}_[a-zA-Z0-9]{59}|v[0-9]\.[0-9a-f]{40})$").unwrap();
    static ref REGEX_GITHUB_SRC_PATH: regex::Regex = regex::Regex::new(r"^src/bo4e_schemas/(?P<module>.*)\.json$").unwrap();
}

pub fn is_valid_github_token(token: &str) -> bool {
    REGEX_GITHUB_TOKEN.is_match(token)
}

pub fn get_token_from_github_cli() -> Option<String> {
    std::process::Command::new("gh")
        .arg("auth")
        .arg("token")
        .output()
        .ok()
        .and_then(|output| output.status.success().then(|| output))
        .and_then(|output| {
            let token_str = String::from_utf8_lossy(&output.stdout);
            let token_str = token_str.trim();
            is_valid_github_token(token_str).then(|| token_str.to_string())
        })
}

type AsyncInvokeLater<T> = Pin<Box<dyn Future<Output = T>>>;

async fn _get_schemas_from_github_recursive(
    octocrab: octocrab::Octocrab,
    target_commitish: String,
    dir_path: String,
) -> Result<Vec<AsyncInvokeLater<Result<Schema, String>>>, String> {
    let items = get_bo4e_schemas_repo_handler(&octocrab)
        .get_content()
        .r#ref(target_commitish.clone())
        .path(dir_path)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    let mut futures: Vec<AsyncInvokeLater<Result<Schema, String>>> = Vec::new();

    for item in items.items {
        match item.r#type.as_str() {
            "file" => {
                if let Some(path_match) = REGEX_GITHUB_SRC_PATH.captures(&item.path) {
                    let octocrab = octocrab.clone();
                    let target_commitish = target_commitish.clone();
                    let file_path = item.path.clone();
                    let path_slice = path_match.name("module").unwrap().as_str().to_string();

                    futures.push(Box::pin(async move {
                        let file_content = get_bo4e_schemas_repo_handler(&octocrab)
                            .get_content()
                            .r#ref(target_commitish)
                            .path(file_path)
                            .send()
                            .await
                            .map_err(|e| e.to_string())?
                            .items[0]
                            .decoded_content()
                            .ok_or("Failed to retrieve and decode file content".to_string())?;
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
                        octocrab.clone(),
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
    enable_output: bool,
) -> Result<Vec<T>, String> {
    let total = futures.len();
    let start_message = "Downloading schemas...";
    let finish_message = "Downloaded schemas.   ";
    let pb = enable_output.then(|| new_progress_bar(total as u64, Some(start_message.to_string())));

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

fn get_octocrab_instance(token: Option<&str>) -> Result<octocrab::Octocrab, String> {
    if let Some(token) = token {
        octocrab::Octocrab::builder()
            .personal_token(token.to_string())
            .build()
            .map_err(|e| e.to_string())
    } else {
        octocrab::Octocrab::builder()
            .build()
            .map_err(|e| e.to_string())
    }
}

fn get_bo4e_schemas_repo_handler(octocrab: &octocrab::Octocrab) -> RepoHandler<'_> {
    octocrab.repos("bo4e", "BO4E-Schemas")
}

async fn get_target_commitish_from_tag(
    repo_handler: &RepoHandler<'_>,
    version_tag: &Version,
) -> Result<String, String> {
    let reference = repo_handler
        .releases()
        .get_by_tag(&version_tag.to_string())
        .await
        .map_err(|e| e.to_string())?;
    Ok(reference.target_commitish)
}

/// Query the GitHub API of `bo4e/BO4E-Schemas` for a specific version.
/// Returns metadata of all BO4E schemas.
// Uses octocrab to interact with the GitHub API.

pub async fn get_schemas_from_github(
    version_tag: &Version,
    token: Option<&str>,
    enable_output: bool,
) -> Result<Schemas, String> {
    let octocrab = get_octocrab_instance(token)?;
    let target_commitish =
        get_target_commitish_from_tag(&get_bo4e_schemas_repo_handler(&octocrab), version_tag)
            .await?;

    let schema_downloads = _get_schemas_from_github_recursive(
        octocrab,
        target_commitish,
        "src/bo4e_schemas".to_string(),
    )
    .await?;
    let local_set = tokio::task::LocalSet::new();
    let schemas_vector = local_set
        .run_until(_execute_futures_with_progress_bar(
            schema_downloads,
            enable_output,
        ))
        .await?
        .into_iter()
        .collect::<Result<Vec<Schema>, String>>()?;
    let schemas = Schemas::try_from((schemas_vector, version_tag.into()))?;

    Ok(schemas)
}

pub async fn resolve_latest_version(token: Option<&str>) -> Result<Version, String> {
    let octocrab = get_octocrab_instance(token)?;
    let latest_release = get_bo4e_schemas_repo_handler(&octocrab)
        .releases()
        .get_latest()
        .await
        .map_err(|e| e.to_string())?;
    Version::from_str(&latest_release.tag_name)
}

/// Check if a GitHub *Release* exists in `bo4e/BO4E-Schemas` for the given version.
///
/// Note: a Release is more than a pushed tag. A tag without an associated Release
/// returns 404 from `releases().get_by_tag(...)` and is treated as `Ok(false)`.
pub async fn release_exists(version: &Version, token: Option<&str>) -> Result<bool, String> {
    let octocrab = get_octocrab_instance(token)?;
    match get_bo4e_schemas_repo_handler(&octocrab)
        .releases()
        .get_by_tag(&version.to_string())
        .await
    {
        Ok(_) => Ok(true),
        Err(octocrab::Error::GitHub { source, .. })
            if source.status_code == http::StatusCode::NOT_FOUND =>
        {
            Ok(false)
        }
        Err(octocrab::Error::GitHub { source, .. })
            if source.status_code == http::StatusCode::FORBIDDEN =>
        {
            Err(format!(
                "GitHub rate-limited the release-validation request ({}). \
                 Pass --token, set GITHUB_TOKEN, or use --no-validate-releases.",
                source.message
            ))
        }
        Err(e) => Err(e.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Compile-time check that `release_exists` has the expected signature.
    /// We can't make a live network call in unit tests.
    #[test]
    fn test_release_exists_signature() {
        fn _assert_signature(v: &Version) -> impl std::future::Future<Output = Result<bool, String>> + '_ {
            release_exists(v, None)
        }
        let _ = _assert_signature; // silence unused
    }
}
