"""
This module provides functions to create and read the version file.
"""

from pathlib import Path

from bo4e_cli.models.version import Version


def create_version_file(output_dir: Path, version: Version) -> None:
    """
    Create a version file.
    """
    (output_dir / ".version").write_text(str(version), encoding="utf-8")


def read_version_file(input_dir: Path) -> Version:
    """
    Read the version file.
    """
    return Version.from_str((input_dir / ".version").read_text(encoding="utf-8").strip())
