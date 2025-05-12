"""
Contains the classes to model changes between two BO4E versions.
"""

from typing import Iterable

from bo4e_cli.models.changes import Change, ChangeType


def is_change_critical(change: Change) -> bool:
    """
    This function checks if a change is critical i.e. if the new value is incompatible to the old value.
    """
    return change.type in (
        ChangeType.FIELD_REMOVED,
        ChangeType.FIELD_TYPE_CHANGED,
        ChangeType.FIELD_CARDINALITY_CHANGED,
        ChangeType.FIELD_REFERENCE_CHANGED,
        ChangeType.FIELD_STRING_FORMAT_CHANGED,
        ChangeType.FIELD_ANY_OF_TYPE_ADDED,
        ChangeType.FIELD_ANY_OF_TYPE_REMOVED,
        ChangeType.FIELD_ALL_OF_TYPE_ADDED,
        ChangeType.FIELD_ALL_OF_TYPE_REMOVED,
        ChangeType.CLASS_REMOVED,
        ChangeType.ENUM_VALUE_REMOVED,
    )


def filter_non_crit(changes: Iterable[Change]) -> Iterable[Change]:
    """
    This function filters out all non-critical changes.
    """
    return (change for change in changes if is_change_critical(change))
