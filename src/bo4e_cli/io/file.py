"""
This module contains functions to read or write files to the file system.
"""

from pathlib import Path
from typing import Optional

from rich.progress import track

from bo4e_cli.io.console import CONSOLE


def write_file_contents(
    files: dict[Path, str], *, base_path: Optional[Path] = None, enable_tracker: bool = True
) -> None:
    """
    Write the files to the output directory
    """
    for sub_path, content in track(
        files.items(), description="Writing files...", total=len(files), console=CONSOLE, disable=not enable_tracker
    ):
        file_path = base_path / sub_path if base_path else sub_path
        file_path.parent.mkdir(parents=True, exist_ok=True)
        file_path.write_text(content, encoding="utf-8")
