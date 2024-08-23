"""
This module provides functions to interact with the GitHub API.
"""

import asyncio
from functools import lru_cache
from pathlib import Path

import httpx
from github import Github
from github.Auth import Token
from github.Repository import Repository

# pylint: disable=redefined-builtin
from rich import print
from rich.progress import track

from bo4e_cli.models.github import SchemaMeta, Schemas, SchemaTree
from bo4e_cli.models.schema import SchemaRootType

OWNER = "bo4e"
REPO = "BO4E-Schemas"
TIMEOUT = 10  # in seconds


@lru_cache(maxsize=1)
def get_source_repo(token: str | None) -> Repository:
    """
    Get the source repository.
    """
    if token is not None:
        return Github(auth=Token(token)).get_repo(f"{OWNER}/{REPO}")
    return Github().get_repo(f"{OWNER}/{REPO}")


def resolve_latest_version(token: str | None) -> str:
    """
    Resolve the latest BO4E version from the github api.
    """
    repo = get_source_repo(token)
    latest_release = repo.get_latest_release().title
    return latest_release


def get_schemas_meta_from_gh(version: str, token: str | None) -> Schemas:
    """
    Query the github tree api for a specific package and version.
    """
    print(f"Querying GitHub tree for version {version}")
    repo = get_source_repo(token)
    release = repo.get_release(version)
    tree = repo.get_git_tree(release.target_commitish, recursive=True)
    schemas = Schemas()

    for tree_element in tree.tree:
        if not tree_element.path.startswith("src/bo4e_schemas"):
            continue
        if tree_element.path.endswith(".json"):
            # We could send a `get_contents` request for each file, but instead we send a request
            # for the respective parent directory. This way we only need one request per directory.
            continue
        contents = repo.get_contents(tree_element.path, ref=release.target_commitish)
        if not isinstance(contents, list):
            contents = [contents]
        for file_or_dir in contents:
            if file_or_dir.name.endswith(".json"):
                relative_path = Path(file_or_dir.path).relative_to("src/bo4e_schemas").with_suffix("")
                schema = SchemaMeta(
                    name=file_or_dir.name,
                    module=relative_path.parts,
                    src=file_or_dir.download_url,
                )
                schemas.add(schema)
    return schemas


async def download(schema: SchemaMeta, client: httpx.AsyncClient, token: str | None) -> str:
    """
    Download the schema file.
    """
    if token is not None:
        headers = {"Authorization": f"Bearer {token}"}
    else:
        headers = None
    response = await client.get(schema.src_url, timeout=TIMEOUT, headers=headers)
    response.encoding = "utf-8"

    if response.status_code != 200:
        raise ValueError(f"Could not download schema from {schema.src_url}: {response.text}")
    return response.text


async def download_schemas(output_dir: Path, version: str, token: str | None) -> Schemas:
    """
    Download all schemas.
    """
    schemas = get_schemas_meta_from_gh(version, token)
    async with httpx.AsyncClient() as client:

        async def download_and_save(schema: SchemaMeta):
            schema_text = await download(schema, client, token)
            print("Downloaded %s from %s", schema.name, schema.src_url)
            schema.schema_parsed = SchemaRootType.model_validate_json(schema_text)

        tasks = {download_and_save(schema) for schema in schemas}
        for task in track(asyncio.as_completed(tasks), description="Downloading schemas...", total=len(schemas)):
            await task

    # print(f"All schemas have been downloaded to {output_dir}")
    # version_file = output_dir / ".version"
    # version_file.write_text(version, encoding="utf-8")
    # print(f"Version {version} written to {version_file}")
    return schemas
