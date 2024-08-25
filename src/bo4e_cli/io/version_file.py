"""
This module provides functions to create and read the version file.
"""
from pathlib import Path


def create_version_file(output_dir: Path, version: str) -> None:
    """
    Create a version file.
    """
    (output_dir / ".version").write_text(version, encoding="utf-8")


def read_version_file(output_dir: Path) -> str:
    """
    Read the version file.
    """
    return (output_dir / ".version").read_text(encoding="utf-8").strip()
