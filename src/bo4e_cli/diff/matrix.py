"""
This module contains the logic to create the compatibility matrix from a list of changes.
"""

import csv
import itertools
from enum import StrEnum
from pathlib import Path
from typing import Mapping, Sequence

from bo4e_cli.diff.filters import is_change_critical
from bo4e_cli.models.changes import Change, Changes, ChangeType
from bo4e_cli.models.meta import Schemas, Version


class ChangeSymbol(StrEnum):
    """
    This enum class lists the different symbols of changes in the compatibility matrix.
    """

    CHANGE_NONE = "🟢"
    CHANGE_NON_CRITICAL = "🟡"
    CHANGE_CRITICAL = "🔴"
    NON_EXISTENT = "\\-"
    ADDED = "➕"
    REMOVED = "➖"


def determine_symbol(changes: Sequence[Change], schemas: Schemas, cls: tuple[str, ...]) -> ChangeSymbol:
    """
    Determine the symbol of a change.
    """
    if len(changes) == 1 and changes[0].type == ChangeType.CLASS_REMOVED:
        return ChangeSymbol.REMOVED
    if len(changes) == 1 and changes[0].type == ChangeType.CLASS_ADDED:
        return ChangeSymbol.ADDED
    if cls not in schemas.modules:
        return ChangeSymbol.NON_EXISTENT
    if len(changes) == 0:
        return ChangeSymbol.CHANGE_NONE

    assert all(
        change.type not in (ChangeType.CLASS_ADDED, ChangeType.CLASS_REMOVED) for change in changes
    ), "Internal error: CLASS_ADDED and CLASS_REMOVED must be the only change per class if present."
    if any(is_change_critical(change) for change in changes):
        return ChangeSymbol.CHANGE_CRITICAL
    return ChangeSymbol.CHANGE_NON_CRITICAL


def create_compatibility_matrix_csv(
    output: Path,
    versions: Sequence[Version],
    schemas_per_version: Mapping[Version, Schemas],
    changes_per_versions: Mapping[tuple[Version, Version], Changes],
) -> None:
    """
    Create a compatibility matrix csv file from the given changes.
    """
    output.parent.mkdir(parents=True, exist_ok=True)
    with open(output, "w", encoding="utf-8") as file:
        csv_writer = csv.writer(file, delimiter=",", lineterminator="\n", escapechar="/")
        csv_writer.writerow(("", *versions[1:]))
        all_classes: set[tuple[str, ...]] = set(
            schema.module for schemas in schemas_per_version.values() for schema in schemas
        )

        for class_path in sorted(all_classes, key=lambda cls: tuple(cls_part.lower() for cls_part in cls)):
            row = [class_path[-1]]
            class_path_str = "/".join(class_path) + "#"
            for version_old, version_new in itertools.pairwise(versions):
                changes_related_to_class = [
                    change
                    for change in changes_per_versions[(version_old, version_new)].changes
                    if change.old_trace.startswith(class_path_str) or change.new_trace.startswith(class_path_str)
                ]
                row.append(
                    determine_symbol(changes_related_to_class, schemas_per_version[version_new], class_path).value
                )
            csv_writer.writerow(row)
