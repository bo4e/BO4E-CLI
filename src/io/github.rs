use crate::models::schema_meta::{Schema, SchemaMeta, Schemas, Source};
use crate::models::version::Version;
use lazy_static::lazy_static;
use octocrab::repos::RepoHandler;
use std::rc::Rc;
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
    repo_handler: &RepoHandler<'_>,
    target_commitish: &str,
    path: &str,
    schemas: &mut Schemas,
) -> Result<(), String> {
    let items = repo_handler
        .get_content()
        .r#ref(target_commitish)
        .path(path)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    for item in items.items {
        match item.r#type.as_str() {
            "file" => {
                if let Some(path_match) = REGEX_GITHUB_SRC_PATH.captures(&item.path) {
                    let path_slice = path_match.name("module").unwrap().as_str();
                    //let response = repo_handler.raw_file(target_commitish, &item.path).await?;
                    //response.status()
                    //let schema: Schema = serde_json::from_str(&file_content)
                    //    .map_err(|e| octocrab::Error::Other(e.to_string()))?;
                    let schema = Schema::from(SchemaMeta::new(
                        path_slice.split('/').map(String::from).collect(),
                        Some(Source::Online(Url::parse(path_slice).unwrap())),
                    )?);
                    return schemas.add_schema(Rc::new(schema));
                }
            }
            "dir" => {
                Box::pin(_get_schemas_from_github_recursive(
                    repo_handler,
                    target_commitish,
                    &item.path,
                    schemas,
                ))
                .await?;
            }
            _ => {
                // Ignore other types (e.g., symlinks, submodules)
            }
        }
    }
    Ok(())
}

pub fn get_octocrab_instance(token: Option<&str>) -> Result<octocrab::Octocrab, String> {
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

pub fn get_bo4e_schemas_repo_handler(octocrab: &octocrab::Octocrab) -> RepoHandler<'_> {
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
    repo_handler: &RepoHandler<'_>,
    version_tag: &Version,
    target_commitish: &str,
) -> Result<Schemas, String> {
    // Get the reference for the tag

    let mut schemas = Schemas::new(version_tag.into());
    repo_handler.raw_file()
    _get_schemas_from_github_recursive(
        repo_handler,
        target_commitish,
        "src/bo4e_schemas",
        &mut schemas,
    )
    .await?;

    Ok(schemas)
}

pub async fn download_schemas_from_github(schemas: &mut Schemas) -> Result<(), String> {
    for schema in schemas {
        if let Some(download_url) = schema.src_url() {
            // TODO: make a GET request to download_url with crate `http`

            schema.load_schema(schema_text);
        } else {
            return Err(format!(
                "Schema {} does not have a valid online source URL.",
                schema.name()
            ));
        }
    }
    Ok(())
}
