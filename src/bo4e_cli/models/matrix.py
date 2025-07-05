"""
Contains a class to model the compatibility matrix of BO4E versions.
"""

from pydantic import BaseModel, Field

from bo4e_cli.models.changes import ChangeSymbol, ChangeText
from bo4e_cli.models.meta import Version
from bo4e_cli.utils.data_structures import RootModelDict


class CompatibilityMatrixEntry(BaseModel):
    """
    This class models a single entry in the compatibility matrix.
    It contains the compatibility status and the corresponding change text.
    """

    previous_version: Version
    next_version: Version
    compatibility: ChangeText | ChangeSymbol


class CompatibilityMatrix(RootModelDict[str, list[CompatibilityMatrixEntry]]):
    """
    This class models the compatibility matrix of BO4E versions.
    """

    root: dict[str, list[CompatibilityMatrixEntry]] = Field(default_factory=dict)
