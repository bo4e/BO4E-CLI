"""
Contains logic related to cleansing io operations.
"""

import shutil
from pathlib import Path

from rich import print

from bo4e_cli.io.progress import Routine, track_single


def clear_dir_if_needed(directory: Path) -> None:
    """
    Clear (and delete) the directory if needed.
    """
    if directory.is_dir() and any(directory.iterdir()):
        track_single(
            Routine(shutil.rmtree, directory),
            description=f"Clearing directory [bold #8cc04d]{directory}[/]",
            finish_description=lambda _: f"Cleared directory [bold #8cc04d]{directory}[/]",
        )
    if directory.exists():
        directory.unlink()
