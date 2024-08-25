"""
Contains parser functions for custom types in the CLI.
"""

from rich import print

from bo4e_cli.io.github import resolve_latest_version
from bo4e_cli.models.meta import Version


def parse_version(version: str) -> Version:
    """
    Parse a version string.
    """
    if version == "latest":
        latest_version = resolve_latest_version(token=None)
        print(f"Resolved latest release to [bold #8cc04d]{latest_version}[/]")
        return latest_version
    return Version.from_str(version)
