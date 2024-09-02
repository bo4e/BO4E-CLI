"""
Contains a dummy function to prevent linter errors while only introducing the CLI structure.
When filled with logic, this file can be removed.
"""

from typing import Any

from rich import print as print_rich


def dummy(*args: Any, **kwargs: Any) -> None:
    """
    Dummy function to prevent linter errors.
    """
    print_rich("[red]This is a dummy function to prevent linter errors.[/]")
    print_rich(f"Arguments: {args}")
    print_rich(f"Keyword arguments: {kwargs}")
