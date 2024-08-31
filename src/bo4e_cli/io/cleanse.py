"""
Contains logic related to cleansing io operations.
"""

import shutil
from pathlib import Path

from bo4e_cli.io.console.track import Routine, track_single


def clear_dir_if_needed(directory: Path) -> None:
    """
    Clear (and delete) the directory if needed.
    """
    if not directory.exists():
        return
    if not directory.is_dir():
        raise ValueError(f"Expected a directory, got {directory}")
    if any(directory.iterdir()):
        track_single(
            Routine(shutil.rmtree, directory),
            description=f"Clearing directory [bold #8cc04d]{directory}[/]",
            finish_description=lambda _: f"Cleared directory [bold #8cc04d]{directory}[/]",
        )
    else:
        directory.rmdir()
