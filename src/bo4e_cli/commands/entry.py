"""
This module contains the entry point for the CLI.
"""

from importlib.metadata import version as get_version
from typing import Annotated

import typer

from bo4e_cli.io.console.console import CONSOLE

app = typer.Typer(
    rich_markup_mode="rich",
    no_args_is_help=True,
)


@app.callback(no_args_is_help=True, invoke_without_command=True)
def callback(
    show_version: Annotated[
        bool, typer.Option("--version", is_eager=True, show_default=False, help="Print programs current version number")
    ] = False,
    verbose: Annotated[
        bool, typer.Option("--verbose", "-v", show_default=False, help="Print additional information")
    ] = False,
) -> None:
    """
    BO4E - Business Objects for Energy

    This CLI is intended for developers working with BO4E.
    For more information see '--help' or visit
    [link=https://github.com/bo4e/BO4E-CLI?tab=readme-ov-file#bo4e-cli]GitHub[/].
    """
    if show_version:
        CONSOLE.out(f"v{get_version('bo4e-cli')}", highlight=False)
        raise typer.Exit()
    if verbose:
        CONSOLE.verbose = True
    else:
        CONSOLE.verbose = False
