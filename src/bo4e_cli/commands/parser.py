"""
Contains parser functions for custom types in the CLI.
"""

from rich import print

from bo4e_cli.io.github import resolve_latest_version
from bo4e_cli.io.progress import Routine, track_single
from bo4e_cli.models.meta import Version


def parse_version(version: str) -> Version:
    """
    Parse a version string.
    """
    if version == "latest":
        latest_version: Version = track_single(
            Routine(resolve_latest_version, token=None),
            description="Querying GitHub for latest version",
            finish_description=lambda result: f"Resolved latest release to [bold #8cc04d]{result}[/]",
        )
        return latest_version
    version_obj = Version.from_str(version)
    print(f"Using version [bold #8cc04d]{version_obj}[/]")
    return version_obj
