"""
This module contains a function to check version bumps between two sets of BO4E `Schemas`.
"""

from bo4e_cli.io.console import CONSOLE
from bo4e_cli.models.changes import Changes


def check_version_bump(changes: Changes, *, major_bump_allowed: bool = True) -> None:
    """
    Check if the new version of the schemas is a valid bump from the old version.
    If the versions have
    """

    version_old = changes.old_version
    version_new = changes.new_version

    assert version_new > version_old, f"Version did not increase: {version_new} <= {version_old}"

    CONSOLE.print('"New" version is ahead of the compared "old" version.', show_only_on_verbose=True)
    if version_new.bumped_major(version_old):
        if not major_bump_allowed:
            raise ValueError("Major bump detected. Major bump is not allowed due to set argument flag.")
        CONSOLE.print("Major version bump detected. No further checks needed.", show_only_on_verbose=True)
        return
    CONSOLE.print("Check if functional or technical release bump is needed", show_only_on_verbose=True)
    functional_changes = len(changes.changes) > 0
    CONSOLE.print(
        f"{"Functional" if functional_changes else "Technical"} release bump is needed", show_only_on_verbose=True
    )

    if not functional_changes and version_new.bumped_functional(version_old):
        raise ValueError(
            "Functional version bump detected but no functional changes found. "
            "Please bump the technical release count instead of the functional."
        )
    if functional_changes and not version_new.bumped_functional(version_old):
        raise ValueError(
            "No functional version bump detected but functional changes found. "
            "Please bump the functional release count.\n"
            f"Detected changes: {changes.model_dump_json(indent=2)}"
        )
