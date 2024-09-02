"""
This module contains autocompletion functions for the CLI.
"""

import os
from typing import Iterable

import typer

from bo4e_cli.io.github import get_versions


def version_autocompletion(ctx: typer.Context) -> Iterable[str]:
    """
    Autocompletion function for the version tag.
    It will try to retrieve a GitHub access token from the environment variable `GITHUB_ACCESS_TOKEN` if no token
    is provided as a parameter yet. But it can be used without a token as well.
    """
    incomplete_value = ctx.params.get("version_tag", "")
    token = ctx.params.get("token", None)
    if token is None:
        token = os.environ.get("GITHUB_ACCESS_TOKEN", None)

    # Note that token can still be None. For documentation on this see help text of `bo4e pull` command.
    for release in get_versions(token=token):
        version = str(release)
        if version.startswith(incomplete_value):
            yield version
