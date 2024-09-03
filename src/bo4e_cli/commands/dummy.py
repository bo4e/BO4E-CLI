"""
Contains a dummy function to prevent linter errors while only introducing the CLI structure.
When filled with logic, this file can be removed.
"""

import json
from typing import Any

from rich.highlighter import JSONHighlighter

from bo4e_cli.io.console import CONSOLE


def serializer(obj: Any) -> Any:
    """
    Serialize an object.
    """
    return str(obj)


def dummy(*args: Any, **kwargs: Any) -> None:
    """
    Dummy function to prevent linter errors.
    """
    CONSOLE.print("[red]This is a dummy function to prevent linter errors.[/]")
    CONSOLE.print("Arguments:", JSONHighlighter()(json.dumps(args, default=serializer)))
    CONSOLE.print("Keyword arguments:", JSONHighlighter()(json.dumps(kwargs, default=serializer)))
