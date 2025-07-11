"""
Returns some utility functions to interact with the GitHub CLI.
"""

import re
import subprocess

from bo4e_cli.io.console import CONSOLE

REGEX_GH_TOKEN = re.compile(
    r"^(gh[pousr]_[A-Za-z0-9_]{36,251}|github_pat_[a-zA-Z0-9]{22}_[a-zA-Z0-9]{59}|v[0-9]\.[0-9a-f]{40})$"
)
# pylint: disable=line-too-long
# Taken from https://gist.github.com/magnetikonline/073afe7909ffdd6f10ef06a00bc3bc88?permalink_comment_id=4577400#gistcomment-4577400


def is_github_cli_installed() -> bool:
    """Check if the GitHub CLI is installed."""
    try:
        result = subprocess.run(["gh", "--version"], capture_output=True, check=False)
        return result.returncode == 0
    except FileNotFoundError:
        # The GitHub CLI is not installed or at least not found in the PATH under "gh".
        return False


def get_access_token_from_cli() -> str | None:
    """
    Get the access token from the GitHub CLI.
    Assumes that the GitHub CLI is installed.
    Returns `None` if no user is logged into the GitHub CLI.
    """
    result = subprocess.run(["gh", "auth", "token"], capture_output=True, check=False)
    if result.returncode != 0:
        return None
    token = result.stdout.decode().strip()
    if REGEX_GH_TOKEN.fullmatch(token):
        CONSOLE.print("Retrieved access token from GitHub CLI command `gh auth token`.")
        return token
    return None


def get_access_token_from_cli_if_installed() -> str | None:
    """
    Get the access token from the GitHub CLI if it is installed.
    Returns `None` if the GitHub CLI is not installed or if no user is logged into the GitHub CLI.
    """
    if is_github_cli_installed():
        return get_access_token_from_cli()
    return None
