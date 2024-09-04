"""
This module contains the config model for the `bo4e edit` command.
"""

import re
from typing import Annotated, Literal

from pydantic import BaseModel, Field, field_validator

from bo4e_cli.models.schema import Reference, SchemaRootObject, SchemaRootStrEnum, SchemaType


class AdditionalField(BaseModel):
    """
    A field that is added to the schema
    """

    pattern: str
    field_name: Annotated[str, Field(alias="fieldName")]
    field_def: Annotated[SchemaType, Field(alias="fieldDef")]

    @field_validator("pattern")
    @classmethod
    def validate_pattern(cls, pattern: str) -> str:
        """
        Validates if the pattern is compilable as a regular expression
        """
        try:
            re.compile(pattern)
        except re.error as error:
            raise ValueError(f"Invalid regular expression: {pattern}") from error
        return pattern


class AdditionalEnumItem(BaseModel):
    """
    A enum item that is added to the schema
    """

    pattern: str
    items: list[str]

    @field_validator("pattern")
    @classmethod
    def validate_pattern(cls, pattern: str) -> str:
        """
        Validates if the pattern is compilable as a regular expression
        """
        try:
            re.compile(pattern)
        except re.error as error:
            raise ValueError(f"Invalid regular expression: {pattern}") from error
        return pattern


class AdditionalModel(BaseModel):
    """
    A model that is added to the schema
    """

    module: Literal["bo", "com", "enum"]
    schema_parsed: Annotated[SchemaRootObject | SchemaRootStrEnum | Reference, Field(alias="schema")]


class Config(BaseModel):
    """
    The config file model
    """

    non_nullable_fields: Annotated[list[str], Field(alias="nonNullableFields", default_factory=list)]
    additional_fields: Annotated[
        list[AdditionalField | Reference], Field(alias="additionalFields", default_factory=list)
    ]
    additional_enum_items: Annotated[list[AdditionalEnumItem], Field(alias="additionalEnumItems", default_factory=list)]
    additional_models: Annotated[list[AdditionalModel], Field(alias="additionalModels", default_factory=list)]

    @field_validator("non_nullable_fields")
    @classmethod
    def validate_non_nullable_field_patterns(cls, non_nullable_fields: list[str]) -> list[str]:
        """
        Validates if the patterns are compilable as a regular expression
        """
        for pattern in non_nullable_fields:
            try:
                re.compile(pattern)
            except re.error as error:
                raise ValueError(f"Invalid regular expression: {pattern}") from error
        return non_nullable_fields
