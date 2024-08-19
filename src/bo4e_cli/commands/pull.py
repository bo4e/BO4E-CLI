from pathlib import Path
from typing import Annotated, Optional

import typer

from bo4e_cli.commands.entry import app


@app.command()
def pull(
    *,
    output_dir: Annotated[Path, typer.Option(help="The directory to save the JSON-schemas to.", show_default=False)],
    version_tag: Annotated[
        str,
        typer.Option(
            help="The BO4E-version tag to pull the data for. "
            "They will be pulled from https://github.com/bo4e/BO4E-Schemas."
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
            "Use this if you have problems with the rate limit. "
            "Alternatively, you can set the environment variable GITHUB_ACCESS_TOKEN.",
            envvar="GITHUB_ACCESS_TOKEN",
        ),
    ] = None,
):
    """
    Pull all BO4E-JSON-schemas of a specific version.
    Beside the json-files a .version file will be created in utf-8 format at root of the output directory.
    This file is needed for other commands.
    """
    pass
