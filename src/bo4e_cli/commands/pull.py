"""
This module contains the command to pull the BO4E-schemas from the BO4E-Schemas repository.
"""

import asyncio
from pathlib import Path
from typing import Annotated, Optional

import typer

from bo4e_cli.commands.autocompletion import version_autocompletion
from bo4e_cli.commands.entry import app
from bo4e_cli.commands.parser import parse_version
from bo4e_cli.edit.update_refs import update_references_all_schemas
from bo4e_cli.io.cleanse import clear_dir_if_needed
from bo4e_cli.io.console.console import CONSOLE, add_schemas_to_highlighter
from bo4e_cli.io.github import download_schemas
from bo4e_cli.io.schemas import write_schemas
from bo4e_cli.utils.github_cli import get_access_token_from_cli_if_installed


@app.command()
def pull(
    *,
    output_dir: Annotated[
        Path,
        typer.Option(
            "--output", "-o", help="The directory to save the JSON-schemas to.", show_default=False, resolve_path=True
        ),
    ],
    version_tag: Annotated[
        str,
        typer.Option(
            "--version-tag",
            "-t",
            help="The BO4E-version tag to pull the data for. "
            "If none is provided, the latest version will be queried from GitHub. "
            "They will be pulled from https://github.com/bo4e/BO4E-Schemas.",
            autocompletion=version_autocompletion,
        ),
    ] = "latest",
    update_refs: Annotated[
        bool,
        typer.Option(
            help="Automatically update the references in the schemas. "
            "Online references to BO4E-schemas will be replaced by relative paths."
        ),
    ] = True,
    clear_output: Annotated[bool, typer.Option(help="Clear the output directory before saving the schemas.")] = True,
    token: Annotated[
        Optional[str],
        typer.Option(
            help="A GitHub Access token to authenticate with the GitHub API. "
            "Use this if you have rate limiting problems with the GitHub API. "
            "It is encouraged to set the environment variable GITHUB_ACCESS_TOKEN instead to prevent"
            "accidentally storing your token into the shell history. "
            "Alternatively, if you have the GitHub CLI installed and "
            "the token can't be found in the environment variables, "
            "the token will be fetched from the GitHub CLI (if you are logged in). Uses `gh auth token`.",
            envvar="GITHUB_ACCESS_TOKEN",
        ),
    ] = None,
) -> None:
    """
    Pull all BO4E-JSON-schemas of a specific version.

    Beside the json-files a .version file will be created in utf-8 format at root of the output directory.
    This file is needed for other commands.
    """
    if token is not None:
        CONSOLE.print("Using GitHub Access Token for authentication.")
    else:
        token = get_access_token_from_cli_if_installed()
        if token is not None:
            CONSOLE.print("Using GitHub Access Token from GitHub CLI for authentication.")
    if token is None:
        CONSOLE.print(
            "No GitHub Access Token provided. "
            "This may lead to rate limiting issues if you run this command multiple times."
        )
    version = parse_version(version_tag, token=token)
    if clear_output:
        clear_dir_if_needed(output_dir)

    schemas = asyncio.run(download_schemas(version=version, token=token))
    add_schemas_to_highlighter(schemas)
    if update_refs:
        update_references_all_schemas(schemas)
    write_schemas(schemas, output_dir)
