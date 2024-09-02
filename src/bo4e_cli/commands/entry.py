"""
This module contains the entry point for the CLI.
"""

import typer

app = typer.Typer(
    help="[bold][#8cc04d]BO[/][#617d8b]4E[/] - [#8cc04d]Business Objects[/] [#617d8b]for Energy[/][/bold]\n\n"
    "This CLI is intended for developers working with BO4E.\n"
    "For more information see '--help' or visit "
    "[link=https://github.com/bo4e/BO4E-CLI?tab=readme-ov-file#bo4e-cli]GitHub[/].",
    rich_markup_mode="rich",
    no_args_is_help=True,
)
