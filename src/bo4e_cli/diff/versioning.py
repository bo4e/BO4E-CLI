"""
This module provides a CLI to check if a version tag has the expected format we expect in the BO4E repository.
"""

import logging
import subprocess
import sys

import click
from github import Github
from github.Auth import Token
from github.Repository import Repository

from bo4e_cli.io.console import CONSOLE
from bo4e_cli.io.git import get_branches_containing_commit, get_last_version_before
from bo4e_cli.models.meta import Version

from . import diff


def get_source_repo(gh_token: str | None = None) -> Repository:
    """
    Get the BO4E-python repository from GitHub.
    """
    if gh_token is not None:
        gh = Github(auth=Token(gh_token))
    else:
        gh = Github()
    return gh.get_repo("bo4e/BO4E-python")


def get_latest_version(gh_token: str | None = None) -> Version:
    """
    Get the release from BO4E-python repository which is marked as 'latest'.
    """
    return Version.from_string(get_source_repo(gh_token).get_latest_release().tag_name)


def ensure_latest_on_main(latest_version: Version, is_cur_version_latest: bool) -> None:
    """
    Ensure that the latest release is on the main branch.
    Will also be called if the currently tagged version is marked as `latest`.
    In this case both versions are equal.

    Note: This doesn't revert the release on GitHub. If you accidentally released on the wrong branch, you have to
    manually mark an old or create a new release as `latest` on the main branch. Otherwise, the publishing workflow
    will fail here.
    """
    commit_id = subprocess.check_output(["git", "rev-parse", f"tags/{latest_version.tag_name}~0"]).decode().strip()
    branches_containing_commit = get_branches_containing_commit(commit_id)
    if "remotes/origin/main" not in branches_containing_commit:
        if is_cur_version_latest:
            raise ValueError(
                f"Tagged version {latest_version} is marked as latest but is not on main branch "
                f"(branches {branches_containing_commit} contain commit {commit_id}).\n"
                "Either tag on main branch or don't mark the release as latest.\n"
                "If you accidentally marked the release as latest please remember to revert it. "
                "Otherwise, the next publish workflow will fail as the latest version is assumed to be on main."
            )
        raise ValueError(
            f"Fatal Error: Latest release {latest_version.tag_name} is not on main branch "
            f"(branches {branches_containing_commit} contain commit {commit_id}).\n"
            "Please ensure that the latest release is on the main branch."
        )


def compare_work_tree_with_latest_version(
    gh_version: str, gh_token: str | None = None, major_bump_allowed: bool = True
) -> None:
    """
    Compare the work tree with the latest release from the BO4E repository.
    If any inconsistency is detected, a Value- or an AssertionError will be raised.
    """
    logger.info("Github Access Token %s", "provided" if gh_token is not None else "not provided")
    cur_version = Version.from_string(gh_version, allow_candidate=True)
    CONSOLE.print(f"Tagged release version: {cur_version}", show_only_on_verbose=True)
    latest_version = get_latest_version(gh_token)
    CONSOLE.print(f"Got latest release version from GitHub: {latest_version}", show_only_on_verbose=True)
    is_cur_version_latest = cur_version == latest_version
    if is_cur_version_latest:
        logger.info("Tagged version is marked as latest.")
    ensure_latest_on_main(latest_version, is_cur_version_latest)
    logger.info("Latest release is on main branch.")

    version_ahead = cur_version
    version_behind = get_last_version_before(cur_version)
    logger.info(
        "Comparing with the version before the tagged release (excluding release candidates): %s",
        version_behind,
    )

    assert version_ahead > version_behind, f"Version did not increase: {version_ahead} <= {version_behind}"

    logger.info(
        "Current version is ahead of the compared version. Comparing versions: %s -> %s",
        version_behind,
        version_ahead,
    )
    if version_ahead.bumped_major(version_behind):
        if not major_bump_allowed:
            raise ValueError("Major bump detected. Major bump is not allowed.")
        logger.info("Major version bump detected. No further checks needed.")
        return
    changes = list(
        diff.compare_bo4e_versions(version_behind.tag_name, version_ahead.tag_name, gh_token=gh_token, from_local=True)
    )
    logger.info("Check if functional or technical release bump is needed")
    functional_changes = len(changes) > 0
    logger.info("%s release bump is needed", "Functional" if functional_changes else "Technical")

    if not functional_changes and version_ahead.bumped_functional(version_behind):
        raise ValueError(
            "Functional version bump detected but no functional changes found. "
            "Please bump the technical release count instead of the functional."
        )
    if functional_changes and not version_ahead.bumped_functional(version_behind):
        raise ValueError(
            "No functional version bump detected but functional changes found. "
            "Please bump the functional release count.\n"
            f"Detected changes: {changes}"
        )


@click.command()
@click.option("--gh-version", type=str, required=True, help="The new version to compare the latest release with.")
@click.option(
    "--gh-token", type=str, default=None, help="GitHub Access token. This helps to avoid rate limiting errors."
)
@click.option(
    "--major-bump-allowed/--major-bump-disallowed",
    is_flag=True,
    default=True,
    help="Indicate if a major bump is allowed. "
    "If it is not allowed, the script will exit with an error if a major bump is detected.",
)
def compare_work_tree_with_latest_version_cli(
    gh_version: str, gh_token: str | None = None, major_bump_allowed: bool = True
) -> None:
    """
    Check a version tag and compare the work tree with the latest release from the BO4E repository.
    Exits with status code 1 iff the version is inconsistent with the commit history or if the detected changes in
    the JSON-schemas are inconsistent with the version bump.
    """
    try:
        compare_work_tree_with_latest_version(gh_version, gh_token, major_bump_allowed)
    except Exception as error:
        logger.error("An error occurred.", exc_info=error)
        raise click.exceptions.Exit(1)
    logger.info("All checks passed.")


if __name__ == "__main__":
    # pylint: disable=no-value-for-parameter
    compare_work_tree_with_latest_version_cli()


def test_compare_work_tree_with_latest_version() -> None:
    """
    Little test function for local testing.
    """
    logging.basicConfig(level=logging.DEBUG, stream=sys.stdout)
    compare_work_tree_with_latest_version("v202401.1.2-rc3", gh_token=None)


def test_version() -> None:
    """
    Test the total ordering of the Version class.
    """
    # pylint: disable=unnecessary-negation
    assert Version.from_string("v202401.1.2") == Version(major=202401, functional=1, technical=2)
    assert Version.from_string("v202401.1.2-rc3", allow_candidate=True) == Version(
        major=202401, functional=1, technical=2, candidate=3
    )
    assert Version.from_string("v202401.1.2") < Version.from_string("v202401.1.3")
    assert Version.from_string("v202401.1.2") < Version.from_string("v202401.2.0")
    assert not Version.from_string("v202401.2.0") < Version.from_string("v202401.1.2")
    assert Version.from_string("v202401.2.0") > Version.from_string("v202401.1.2")
    assert Version.from_string("v202401.1.2-rc3", allow_candidate=True) < Version.from_string("v202401.1.2")
    assert Version.from_string("v202401.1.2-rc3", allow_candidate=True) <= Version.from_string("v202401.1.2")
    assert not Version.from_string("v202401.1.2-rc3", allow_candidate=True) >= Version.from_string("v202401.1.2")
    assert Version.from_string("v202401.1.2-rc3", allow_candidate=True) > Version.from_string("v202401.1.1")
    assert Version.from_string("v202401.1.2-rc3", allow_candidate=True) > Version.from_string(
        "v202401.1.2-rc1", allow_candidate=True
    )
