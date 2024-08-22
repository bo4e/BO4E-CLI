"""
This module contains classes to model json files which are formatted as "json schema validation":
https://json-schema.org/draft/2019-09/json-schema-validation
Note that this actually supports mainly our BO4E-Schemas, but not necessarily the full json schema validation standard.
"""

from typing import Annotated
from typing import Any as _Any
from typing import Literal, Optional, Union

from pydantic import AliasChoices, BaseModel, ConfigDict, Field


class TypeBase(BaseModel):
    """
    This pydantic class models the base of a type definition in a json schema validation file.
    """

    description: str = ""
    title: str = ""
    default: _Any = None

    model_config = ConfigDict(populate_by_name=True)


class SchemaRootTypeBase(BaseModel):
    """
    This pydantic class models the base of a root type definition in a json schema validation file.
    The root type may contain special keys like "$defs" or "$schema". Currently, only "$defs" is supported.
    """

    defs: dict[str, "SchemaClassType"] = Field(
        validation_alias=AliasChoices("$defs", "$definitions"),
        serialization_alias="$defs",
        default_factory=dict,
    )

    model_config = ConfigDict(populate_by_name=True)


class Object(TypeBase):
    """
    This pydantic class models the root of a json schema validation file.
    """

    additional_properties: Annotated[Literal[True, False], Field(alias="additionalProperties")] = False
    properties: dict[str, "SchemaType"]
    type: Literal["object"]
    required: list[str] = Field(default_factory=list)


class StrEnum(TypeBase):
    """
    This pydantic class models the "enum" keyword in a json schema validation file.
    """

    enum: list[str]
    type: Literal["string"]


class SchemaRootObject(Object, SchemaRootTypeBase):
    """
    This pydantic class models the root of a json schema validation file as an object type.
    """


class SchemaRootStrEnum(StrEnum, SchemaRootTypeBase):
    """
    This pydantic class models the root of a json schema validation file as an enum type.
    """


class Array(TypeBase):
    """
    This pydantic class models the "array" type in a json schema validation file.
    """

    items: "SchemaType"
    type: Literal["array"]


class AnyOf(TypeBase):
    """
    This pydantic class models the "anyOf" keyword in a json schema validation file.
    """

    any_of: Annotated[list["SchemaType"], Field(alias="anyOf")]


class AllOf(TypeBase):
    """
    This pydantic class models the "allOf" keyword in a json schema validation file.
    """

    all_of: Annotated[list["SchemaType"], Field(alias="allOf")]


class String(TypeBase):
    """
    This pydantic class models the "string" type in a json schema validation file.
    """

    type: Literal["string"]
    format: Optional[
        Literal[
            "date-time",
            "date",
            "time",
            "email",
            "hostname",
            "ipv4",
            "ipv6",
            "uri",
            "uri-reference",
            "iri",
            "iri-reference",
            "uuid",
            "json-pointer",
            "relative-json-pointer",
            "regex",
            "idn-email",
            "idn-hostname",
            "binary",
        ]
    ] = None


class Number(TypeBase):
    """
    This pydantic class models the "number" type in a json schema validation file.
    """

    type: Literal["number"]


class Decimal(TypeBase):
    """
    This pydantic class models the "decimal" type in a json schema validation file.
    """

    type: Literal["string", "number"]
    format: Literal["decimal"]


class Integer(TypeBase):
    """
    This pydantic class models the "integer" type in a json schema validation file.
    """

    type: Literal["integer"]


class Boolean(TypeBase):
    """
    This pydantic class models the "boolean" type in a json schema validation file.
    """

    type: Literal["boolean"]


class Null(TypeBase):
    """
    This pydantic class models the "null" type in a json schema validation file.
    """

    type: Literal["null"]


class Any(TypeBase):
    """
    This pydantic class models the "any" type in a json schema validation file.
    """


class Reference(TypeBase):
    """
    This pydantic class models the "$ref" keyword in a json schema validation file.
    """

    ref: Annotated[str, Field(alias="$ref")]


SchemaType = Union[
    Object, StrEnum, Array, AnyOf, AllOf, String, Decimal, Integer, Number, Boolean, Null, Reference, Any
]
SchemaClassType = Union[Object, StrEnum]
SchemaRootType = Union[SchemaRootObject, SchemaRootStrEnum]
SchemaRootTypeBase.model_rebuild()
SchemaRootObject.model_rebuild()
SchemaRootStrEnum.model_rebuild()
