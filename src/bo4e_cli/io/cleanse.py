import shutil
from pathlib import Path


def clear_dir_if_needed(directory: Path) -> None:
    """
    Clear the directory if needed.
    """
    if directory.is_dir() and any(directory.iterdir()):
        shutil.rmtree(directory)
