"""
Contains parser functions for custom types in the CLI.
"""

from bo4e_cli.models.meta import Version


def parse_version(version: str) -> Version:
    """
    Parse a version string.
    """
    return Version.from_str(version)
