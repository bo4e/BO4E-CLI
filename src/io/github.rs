use crate::models::schema_meta::{Schema, Schemas};
use crate::models::version::Version;
use lazy_static::lazy_static;
use octocrab::repos::RepoHandler;
use serde::de::IntoDeserializer;
use std::rc::Rc;
use std::str::FromStr;
use tokio::task::JoinSet;
use url::Url;

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

async fn _get_schemas_from_github_recursive(
    octocrab: octocrab::Octocrab,
    target_commitish: String,
    dir_path: String,
) -> Result<Vec<Schema>, String> {
    let items = get_bo4e_schemas_repo_handler(&octocrab)
        .get_content()
        .r#ref(target_commitish.clone())
        .path(dir_path)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    let mut join_handle: JoinSet<Result<Vec<Schema>, String>> = JoinSet::new();

    for item in items.items {
        match item.r#type.as_str() {
            "file" => {
                if let Some(path_match) = REGEX_GITHUB_SRC_PATH.captures(&item.path) {
                    let octocrab = octocrab.clone();
                    let target_commitish = target_commitish.clone();
                    let file_path = item.path.clone();
                    let path_slice = path_match.name("module").unwrap().as_str().to_string();

                    join_handle.spawn_local(async move {
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
                        Ok(vec![schema])
                    });
                }
            }
            "dir" => {
                join_handle.spawn_local(Box::pin(_get_schemas_from_github_recursive(
                    octocrab.clone(),
                    target_commitish.clone(),
                    item.path.clone(),
                )));
            }
            _ => {
                // Ignore other types (e.g., symlinks, submodules)
            }
        }
    }
    let mut output = Vec::new();
    while let Some(res) = join_handle.join_next().await {
        match res {
            Ok(Ok(schemas)) => output.extend(schemas),
            Ok(Err(err)) => return Err(err),
            Err(err) if err.is_panic() => return Err(format!("Panic occurred: {:?}", err)),
            Err(err) => return Err(format!("Task joining failed: {:?}", err)),
        }
    }
    Ok(output)
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
) -> Result<Schemas, String> {
    let octocrab = get_octocrab_instance(token)?;
    let target_commitish =
        get_target_commitish_from_tag(&get_bo4e_schemas_repo_handler(&octocrab), version_tag)
            .await?;

    let schemas_vector = _get_schemas_from_github_recursive(
        octocrab,
        target_commitish,
        "src/bo4e_schemas".to_string(),
    )
    .await?;
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
