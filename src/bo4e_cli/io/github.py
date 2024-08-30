"""
This module provides functions to interact with the GitHub API.
"""

import asyncio
import re
from functools import lru_cache
from pathlib import Path
from typing import Callable

import httpx
from github import Github
from github.Auth import Token
from github.Repository import Repository

# pylint: disable=redefined-builtin
from rich import print
from rich.progress import BarColumn, Progress, TaskProgressColumn, TextColumn, TimeRemainingColumn

from bo4e_cli.io.progress import Routine, track_single
from bo4e_cli.models.meta import SchemaMeta, Schemas, Version

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


def resolve_latest_version(token: str | None) -> Version:
    """
    Resolve the latest BO4E version from the github api.
    """
    repo = get_source_repo(token)
    latest_release = repo.get_latest_release().title
    return Version.from_str(latest_release)


def get_versions(token: str | None) -> list[Version]:
    """
    Get all BO4E versions matching the new versioning schema (e.g. v202401.0.1-rc8) from the github api.
    """
    regex = re.compile(r"^v\d{6}\.\d+\.\d+(?:-rc\d*)?$")
    repo = get_source_repo(token)
    releases = repo.get_releases()
    return [Version.from_str(release.title) for release in releases if regex.fullmatch(release.title) is not None]


def get_schemas_meta_from_gh(version: Version, token: str | None) -> Schemas:
    """
    Query the github tree api for a specific package and version.
    Returns metadata of all BO4E schemas.
    """
    repo = get_source_repo(token)
    release = repo.get_release(str(version))
    tree = repo.get_git_tree(release.target_commitish, recursive=True)
    schemas = Schemas(version=version)

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
                    name=relative_path.name,
                    module=relative_path.parts,
                    src=file_or_dir.download_url,
                )
                schemas.add(schema)
    return schemas


async def download(schema: SchemaMeta, client: httpx.AsyncClient, token: str | None) -> str:
    """
    Download the schema file.
    Assumes that the schemas 'src' is a URL (an error will be raised otherwise).
    """
    if token is not None:
        headers = {"Authorization": f"Bearer {token}"}
    else:
        headers = None
    try:
        response = await client.get(str(schema.src_url), timeout=TIMEOUT, headers=headers)
        response.encoding = "utf-8"

        if response.status_code != 200:
            raise ValueError(f"Could not download schema from {schema.src_url}: {response.text}")
        return response.text
    except Exception as e:
        raise ValueError(f"Could not download schema from {schema.src_url}: {e}") from e


async def download_schemas(
    version: Version, token: str | None, callback: Callable[[SchemaMeta], None] | None = None
) -> Schemas:
    """
    Download all schemas. Also prints some output to track the progress.
    A callback can be provided to process the schemas after downloading (to use the power of async).
    """
    schemas = track_single(
        Routine(get_schemas_meta_from_gh, version, token),
        description=f"Querying GitHub tree",
        finish_description=lambda result: f"Queried GitHub tree. Found [bold #8cc04d]{len(result)}[/] schemas.",
    )
    progress = Progress(
        TextColumn("[progress.description]{task.description}"),
        BarColumn(),
        TaskProgressColumn(show_speed=True),
        TimeRemainingColumn(elapsed_when_finished=True),
    )

    with progress:
        async with httpx.AsyncClient(
            transport=httpx.AsyncHTTPTransport(
                limits=httpx.Limits(max_connections=50, max_keepalive_connections=10, keepalive_expiry=10),
                retries=5,
            ),
        ) as client:
            task_id_download = progress.add_task("Downloading schemas...", total=len(schemas))
            if callback is not None:
                task_id_process = progress.add_task("Processing schemas...", total=len(schemas))

            async def download_and_save(schema: SchemaMeta) -> None:
                schema_text = await download(schema, client, token)
                progress.update(task_id_download, advance=1)
                schema.set_schema_text(schema_text)
                if callback is not None:
                    callback(schema)
                    progress.update(task_id_process, advance=1, description=f"Processed {schema.name}")

            tasks = {download_and_save(schema) for schema in schemas}
            await asyncio.gather(*tasks)
            await asyncio.sleep(1)  # This somehow prevents errors from httpx occurring... sometimes

    return schemas
