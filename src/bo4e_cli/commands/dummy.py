"""
Contains a dummy function to prevent linter errors while only introducing the CLI structure.
When filled with logic, this file can be removed.
"""

# pylint: disable=redefined-builtin
from rich import print


def dummy(*args, **kwargs):
    """
    Dummy function to prevent linter errors.
    """
    print("[red]This is a dummy function to prevent linter errors.[/]")
    print(f"Arguments: {args}")
    print(f"Keyword arguments: {kwargs}")
