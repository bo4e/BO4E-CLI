"""
This module provides functions to interact with the git repository in the current working directory.
It is designed to interact with the BO4E repository in order to retrieve version tags, branches, and commits.
"""

import functools
import json
import re
import subprocess
from typing import Iterable, Literal

from bo4e_cli.io.console import CONSOLE
from bo4e_cli.io.github import release_exists
from bo4e_cli.models.version import Version


def is_version_tag(value: str) -> bool:
    """
    Check if value is a valid version tag and exists in repository.
    """
    try:
        Version.from_str(value)
        subprocess.check_call(["git", "show-ref", "--quiet", f"refs/tags/{value}"])
    except (ValueError, subprocess.CalledProcessError):
        return False
    return True


def is_branch(value: str) -> bool:
    """
    Check if a branch is a valid branch name and exists in repository.
    """
    try:
        subprocess.check_call(["git", "show-ref", "--quiet", f"refs/remotes/origin/{value}"])
        return True
    except subprocess.CalledProcessError:
        return False


def is_commit(value: str) -> bool:
    """
    Check if value is a valid commit id.
    """
    try:
        if re.fullmatch(r"^[0-9a-f]{40}$", value) is None:
            return False
        _ = get_branches_containing_commit(value)
        # If the commit ID doesn't exist, an error will be raised.
    except subprocess.CalledProcessError:
        return False
    return True


def get_branches_containing_commit(commit_id: str) -> Iterable[str]:
    """
    Get all branches containing the commit id.
    If the commit id is not found, a subprocess.CalledProcessError will be raised.
    If the commit exists but is not on any branch (e.g. only on tags), an empty Iterable will be returned.
    """
    cmd = ["git", "branch", "-a", "--contains", commit_id]
    output = subprocess.check_output(cmd).decode().strip()
    if output.startswith("error: no such commit"):
        raise subprocess.CalledProcessError(1, cmd, output=output)
    return (line.strip().lstrip("*").lstrip() for line in output.splitlines())


def get_commit_sha(branch_or_tag: str = "HEAD") -> str:
    """
    Get the commit id of the provided reference (branch, tag or "HEAD").
    """
    return subprocess.check_output(["git", "rev-parse", branch_or_tag]).decode().strip()


def get_commit_date(commit_id: str = "HEAD") -> str:
    """
    Get the commit date of the provided commit id (branch, tag or "HEAD").
    The date is returned in ISO 8601 format.
    """
    return subprocess.check_output(["git", "show", "-s", "--format=%ci", commit_id]).decode().strip()


@functools.lru_cache
def get_ref(ref: str) -> tuple[Literal["tag", "branch", "commit"], str]:
    """
    Get the type of reference and the reference itself.
    """
    if is_version_tag(ref):
        CONSOLE.print(f"Get tags before tag {ref}", show_only_on_verbose=True)
        return "tag", ref
    if is_branch(ref):
        CONSOLE.print(f"Get tags on branch {ref}", show_only_on_verbose=True)
        return "branch", ref
    if is_commit(ref):
        CONSOLE.print(f"Get tags before commit {ref}", show_only_on_verbose=True)
        return "commit", ref
    cur_commit = get_commit_sha()
    CONSOLE.print(
        f"Supplied value ({ref}) is neither a tag, a branch nor a commit. "
        f"Get tags before current checkout commit {cur_commit}",
        style="warning",
    )
    return "commit", cur_commit


# pylint: disable=too-many-branches
def get_last_n_tags(
    n: int,
    *,
    ref: str = "main",
    exclude_candidates: bool = True,
    exclude_technical_bumps: bool = False,
    token: str | None = None,
) -> Iterable[Version]:
    """
    Get the last n tags in chronological descending order starting from `ref`.
    If `ref` is a branch, it will start from the current HEAD of the branch.
    If `ref` is a tag, it will start from the tag itself. But the tag itself will not be included in the output.
    If `ref` is neither nor, the main branch will be used as fallback.
    If `exclude_candidates` is True, candidate versions will be excluded from the output.
    If the number of found versions is less than `n`, a warning will be logged.
    If n=0, all versions since v202401.0.0 will be taken into account.
    If exclude_technical_bumps is True, from each functional release group,
    the highest technical release will be returned.
    """
    version_threshold = "v202401.0.0"  # Is used if n=0
    ref_type, reference = get_ref(ref)
    if n == 0:
        CONSOLE.print(f"Get all tags since {version_threshold}", show_only_on_verbose=True)
    else:
        CONSOLE.print(f"Get the last {n} tags", show_only_on_verbose=True)

    CONSOLE.print(f"{'Exclude' if exclude_candidates else 'Include'} release candidates", show_only_on_verbose=True)
    CONSOLE.print(f"{'Exclude' if exclude_technical_bumps else 'Include'} technical bumps", show_only_on_verbose=True)
    output = (
        subprocess.check_output(["git", "tag", "--merged", reference, "--sort=-version:refname", "--sort=-creatordate"])
        .decode()
        .strip()
        .splitlines()
    )
    CONSOLE.print("Output from git tag command:", show_only_on_verbose=True)
    CONSOLE.print_json(json.dumps(output, indent=2), show_only_on_verbose=True)
    output = list(filter(Version.is_valid, output))
    if len(output) == 0:
        CONSOLE.print("No tags found.", style="warning")
        return
    last_version = Version.from_str(output[0])

    counter = 0
    stop_iteration = False
    for ind, tag in enumerate(output):
        if counter >= n > 0:
            stop_iteration = True
        if stop_iteration:
            return
        if n == 0 and tag == version_threshold:
            stop_iteration = True
        version = Version.from_str(tag)
        # pylint: disable=too-many-boolean-expressions
        if (
            exclude_candidates
            and version.is_release_candidate()
            or exclude_technical_bumps
            and ind > 0
            and last_version.bumped_technical(version)
            or ind == 0
            and ref_type == "tag"
        ):
            CONSOLE.print(f"Skipping version {version}", show_only_on_verbose=True)
            continue
        if not release_exists(version, token):
            CONSOLE.print(
                f"Skipping version {version} because it does not exist in the BO4E-Schemas repository.", style="warning"
            )
            continue
        CONSOLE.print(f"Yielding version {version}", show_only_on_verbose=True)
        yield version
        last_version = version
        counter += 1
    if stop_iteration:
        return
    if counter < n and 0 < n:
        if ref_type == "tag":
            CONSOLE.print(f"Only found {counter} tags before tag {ref}, tried to retrieve {n}", style="warning")
        elif ref_type == "branch":
            CONSOLE.print(f"Only found {counter} tags on branch {ref}, tried to retrieve {n}", style="warning")
        else:
            CONSOLE.print(f"Only found {counter} tags before commit {ref}, tried to retrieve {n}", style="warning")
    if n == 0:
        CONSOLE.print(f"Threshold version {version_threshold} not found. Returned all tags.", style="warning")
