"""
This module contains the command for editing JSON-schemas.
"""

from pathlib import Path
from typing import Annotated, Optional

import typer

from bo4e_cli.commands.dummy import dummy
from bo4e_cli.commands.entry import app


@app.command()
def edit(
    *,
    input_dir: Annotated[
        Path, typer.Option("--input", "-i", help="The directory to read the JSON-schemas from.", show_default=False)
    ],
    output_dir: Annotated[
        Path,
        typer.Option("--output", "-o", help="The directory to save the edited JSON-schemas to.", show_default=False),
    ],
    config: Annotated[
        Optional[Path],
        typer.Option("--config", "-c", help="The configuration file to use for editing the JSON-schemas."),
    ] = None,
    set_default_version: Annotated[
        bool,
        typer.Option(
            help="Automatically set or overrides the default version for '_version' fields with the version from "
            ".version file. This is especially useful if you want to define additional models which should "
            "always have the correct version."
        ),
    ] = True,
    clear_output: Annotated[bool, typer.Option(help="Clear the output directory before saving the schemas.")] = True,
) -> None:
    """
    Edit the JSON-schemas in the input directory and save the edited schemas to the output directory.

    The schemas in the input directory won't be changed. If no configuration file is provided, the schemas will be
    copied to the output directory unchanged.
    """
    dummy(
        input_dir=input_dir,
        output_dir=output_dir,
        config=config,
        set_default_version=set_default_version,
        clear_output=clear_output,
    )
