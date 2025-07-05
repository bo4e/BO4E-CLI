"""
Contains io functions for changes json files.
"""

from pathlib import Path
from typing import Iterator

from bo4e_cli.models.changes import Changes


def read_changes_from_diff_files(*file_path: Path) -> Iterator[Changes]:
    """
    Read changes from the given files and yield them one by one.

    :param file_path: The paths to the files containing changes.
    :return: An iterator over Changes objects.
    """
    for path in file_path:
        if not path.exists():
            raise FileNotFoundError(f"File {path} does not exist.")
        yield Changes.model_validate_json(path.read_text(encoding="utf-8"))


def write_changes(changes: Changes, file_path: Path) -> None:
    """
    Write the given changes to the specified file.
    This function creates the parent directories if they do not exist.

    :param changes: The Changes object to write.
    :param file_path: The path to the file where the changes should be written.
    """
    file_path.parent.mkdir(parents=True, exist_ok=True)
    with open(file_path, "w", encoding="utf-8") as file:
        file.write(changes.model_dump_json(indent=2))
