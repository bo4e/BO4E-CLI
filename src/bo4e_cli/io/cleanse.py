"""
Contains logic related to cleansing io operations.
"""

import shutil
from pathlib import Path

from bo4e_cli.io.console import CONSOLE


def clear_dir_if_needed(directory: Path) -> None:
    """
    Clear (and delete) the directory if needed.
    """
    if not directory.exists():
        CONSOLE.print(f"Directory {directory} does not exist, nothing to clear.", show_only_on_verbose=True)
        return
    if not directory.is_dir():
        raise ValueError(f"Expected a directory, got {directory}")
    if any(directory.iterdir()):
        with CONSOLE.status(f"Clearing directory {directory}", spinner="grenade"):
            shutil.rmtree(directory)
        CONSOLE.print(f"Cleared directory {directory}")
    else:
        CONSOLE.print(
            f"Directory {directory} is empty, deleting directory for consistency (will be created later on).",
            show_only_on_verbose=True,
        )
        directory.rmdir()
