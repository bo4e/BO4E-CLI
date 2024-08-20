"""
Contains a dummy function to prevent linter errors while only introducing the CLI structure.
When filled with logic, this file can be removed.
"""

from typing import Any

# pylint: disable=redefined-builtin
from rich import print


def dummy(*args: Any, **kwargs: Any) -> None:
    """
    Dummy function to prevent linter errors.
    """
    print("[red]This is a dummy function to prevent linter errors.[/]")
    print(f"Arguments: {args}")
    print(f"Keyword arguments: {kwargs}")
