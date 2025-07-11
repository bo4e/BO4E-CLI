"""
This module provides functions to interact with the GitHub API.
"""

import asyncio
from functools import lru_cache
from pathlib import Path
from typing import Callable, Iterable

import httpx
from github import Github
from github.Auth import Token
from github.Repository import Repository
from rich.progress import BarColumn, Progress, TaskProgressColumn, TextColumn, TimeRemainingColumn
from rich.text import Text

from bo4e_cli.io.console import CONSOLE
from bo4e_cli.io.console.style import STYLES
from bo4e_cli.models.meta import SchemaMeta, Schemas
from bo4e_cli.models.version import Version

OWNER = "bo4e"
REPO = "BO4E-Schemas"
TIMEOUT = 10  # in seconds


@lru_cache(maxsize=1)
def get_source_repo(token: str | None) -> Repository:
    """
    Get the source repository.
    """
    if token is not None:  # pragma: no cover
        return Github(auth=Token(token)).get_repo(f"{OWNER}/{REPO}")
    return Github().get_repo(f"{OWNER}/{REPO}")


def resolve_latest_version(token: str | None) -> Version:
    """
    Resolve the latest BO4E version from the github api.
    """
    repo = get_source_repo(token)
    latest_release = repo.get_latest_release().title
    return Version.from_str(latest_release)


def get_versions(token: str | None) -> Iterable[Version]:
    """
    Get all BO4E versions matching the new versioning schema (e.g. v202401.0.1-rc8) from the github api.
    """
    repo = get_source_repo(token)
    releases = repo.get_releases()
    for release in releases:
        try:
            yield Version.from_str(release.title)
        except ValueError:
            pass


def release_exists(version: Version, token: str | None) -> bool:
    """
    Check if a release with the given version exists in the BO4E-Schemas repository.
    """
    repo = get_source_repo(token)
    try:
        repo.get_release(str(version))
        return True
    except Exception:  # pylint: disable=broad-exception-caught
        return False


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
                    src=file_or_dir.download_url,  # type: ignore[arg-type]
                )
                schemas.add(schema)

                style = STYLES.get(f"bo4e.{schema.module[0]}", STYLES["bo4e.bo4e_4e"])
                CONSOLE.print(
                    Text.assemble("Found schema ", (schema.name, style), " in ", (str(schema.module), style)),
                    show_only_on_verbose=True,
                )
    return schemas


async def download(schema: SchemaMeta, client: httpx.AsyncClient, token: str | None) -> str:
    """
    Download the schema file.
    Assumes that the schemas 'src' is a URL (an error will be raised otherwise).
    """
    if token is not None:  # pragma: no cover
        headers = {"Authorization": f"Bearer {token}"}
    else:
        headers = None
    try:
        response = await client.get(str(schema.src_url), timeout=TIMEOUT, headers=headers)
        response.encoding = "utf-8"

        if response.status_code != 200:
            raise ValueError(
                f"Could not download schema from {schema.src_url}: {response.status_code}, {response.text}"
            )

        style = STYLES.get(f"bo4e.{schema.module[0]}", STYLES["bo4e.bo4e_4e"])
        CONSOLE.print(
            Text.assemble("Downloaded schema ", (schema.name, style), " from ", (str(schema.src_url), style)),
            show_only_on_verbose=True,
        )
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
    with CONSOLE.status("Querying GitHub tree", spinner="earth"):
        schemas = get_schemas_meta_from_gh(version, token)
    CONSOLE.print(f"Queried GitHub tree. Found {len(schemas)} schemas.")
    progress = Progress(
        TextColumn("[progress.description]{task.description}"),
        BarColumn(),
        TaskProgressColumn(show_speed=True),
        TimeRemainingColumn(elapsed_when_finished=True),
        console=CONSOLE,
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
