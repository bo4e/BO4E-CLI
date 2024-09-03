"""
Contains parser functions for custom types in the CLI.
"""

from bo4e_cli.io.console import CONSOLE
from bo4e_cli.io.github import resolve_latest_version
from bo4e_cli.models.meta import Version


def parse_version(version: str, token: str | None = None) -> Version:
    """
    Parse a version string.
    """
    if version == "latest":
        with CONSOLE.status("Querying GitHub for latest version", spinner="earth"):
            latest_version = resolve_latest_version(token)
        CONSOLE.print(f"Resolved latest release to {latest_version}")
        return latest_version
    version_obj = Version.from_str(version)
    CONSOLE.print(f"Using version {version_obj}")
    return version_obj
