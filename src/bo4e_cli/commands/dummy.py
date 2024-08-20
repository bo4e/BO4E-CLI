"""
Contains a dummy function to prevent linter errors while only introducing the CLI structure.
When filled with logic, this file can be removed.
"""

import typer


def dummy(*args, **kwargs):
    """
    Dummy function to prevent linter errors.
    """
    typer.echo("This is a dummy function to prevent linter errors.")
    typer.echo(f"Arguments: {args}")
    typer.echo(f"Keyword arguments: {kwargs}")
