"""
Contains classes to model the compatibility matrix of BO4E versions.
"""

from enum import StrEnum

from pydantic import BaseModel, Field

from bo4e_cli.models.version import Version
from bo4e_cli.utils.data_structures import RootModelDict


class CompatibilitySymbol(StrEnum):
    """
    This enum class lists the different symbols of changes in the compatibility matrix.
    """

    CHANGE_NONE = "🟢"
    CHANGE_NON_CRITICAL = "🟡"
    CHANGE_CRITICAL = "🔴"
    NON_EXISTENT = "\\-"
    ADDED = "➕"
    REMOVED = "➖"


class CompatibilityText(StrEnum):
    """
    This enum class lists the different text representations of changes in the compatibility matrix.
    """

    CHANGE_NONE = "none"
    CHANGE_NON_CRITICAL = "non\\-critical"
    CHANGE_CRITICAL = "critical"
    NON_EXISTENT = "\\-"
    ADDED = "added"
    REMOVED = "removed"


class CompatibilityMatrixEntry(BaseModel):
    """
    This class models a single entry in the compatibility matrix.
    It contains the compatibility status and the versions related to this change entry.
    """

    previous_version: Version
    next_version: Version
    compatibility: CompatibilityText | CompatibilitySymbol


class CompatibilityMatrix(RootModelDict[str, list[CompatibilityMatrixEntry]]):
    """
    This class models the compatibility matrix of BO4E versions.
    """

    root: dict[str, list[CompatibilityMatrixEntry]] = Field(default_factory=dict)
