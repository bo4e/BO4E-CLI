import shutil
from pathlib import Path

from rich import print


def clear_dir_if_needed(directory: Path) -> None:
    """
    Clear the directory if needed.
    """
    if directory.is_dir() and any(directory.iterdir()):
        print(f"Clearing directory [bold #8cc04d]{directory}[/]")
        shutil.rmtree(directory)
