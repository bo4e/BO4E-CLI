"""
Contains the classes to model changes between two BO4E versions.
"""

from enum import StrEnum
from typing import Any

from pydantic import BaseModel

from bo4e_cli.models.schema import SchemaType


class ChangeType(StrEnum):
    """
    This enum class lists the different types of changes of a single change between two BO4E versions.
    """

    FIELD_ADDED = "field_added"
    FIELD_REMOVED = "field_removed"
    FIELD_DEFAULT_CHANGED = "field_default_changed"
    FIELD_DESCRIPTION_CHANGED = "field_description_changed"
    FIELD_TITLE_CHANGED = "field_title_changed"
    # field type change types
    FIELD_CARDINALITY_CHANGED = "field_cardinality_changed"
    FIELD_REFERENCE_CHANGED = "field_reference_changed"
    FIELD_STRING_FORMAT_CHANGED = "field_string_format_changed"
    FIELD_ANY_OF_TYPE_ADDED = "field_any_of_type_added"
    FIELD_ANY_OF_TYPE_REMOVED = "field_any_of_type_removed"
    FIELD_ALL_OF_TYPE_ADDED = "field_all_of_type_added"
    FIELD_ALL_OF_TYPE_REMOVED = "field_all_of_type_removed"
    FIELD_TYPE_CHANGED = "field_type_changed"  # An arbitrary unclassified change in type

    CLASS_ADDED = "class_added"
    CLASS_REMOVED = "class_removed"
    CLASS_DESCRIPTION_CHANGED = "class_description_changed"

    ENUM_VALUE_ADDED = "enum_value_added"
    ENUM_VALUE_REMOVED = "enum_value_removed"


class Change(BaseModel):
    """
    This pydantic class models a single change between two BO4E versions.
    """

    type: ChangeType
    old: SchemaType | str | None
    new: SchemaType | str | None
    old_trace: str
    new_trace: str

    def __str__(self) -> str:
        return f"{self.type}: {self.old} -> {self.new}"
