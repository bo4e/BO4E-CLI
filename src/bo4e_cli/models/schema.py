"""
This module contains classes to model json files which are formatted as "json schema validation":
https://json-schema.org/draft/2019-09/json-schema-validation
Note that this actually supports mainly our BO4E-Schemas, but not necessarily the full json schema validation standard.
"""

import re
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

    @property
    def python_type_hint(self) -> str:
        """
        Return the python type hint for this type.
        """
        raise NotImplementedError("This method must be implemented by the subclasses.")


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

    @property
    def python_type_hint(self) -> str:
        """
        Return the python type hint for this type.
        """
        if "title" not in self.model_fields_set:
            raise ValueError("Object must have a title.")
        return self.title


class StrEnum(TypeBase):
    """
    This pydantic class models the "enum" keyword in a json schema validation file.
    """

    enum: list[str]
    type: Literal["string"]

    @property
    def python_type_hint(self) -> str:
        """
        Return the python type hint for this type.
        """
        if "title" not in self.model_fields_set:
            raise ValueError("Enum must have a title.")
        return self.title


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

    @property
    def python_type_hint(self) -> str:
        """
        Return the python type hint for this type.
        """
        return f"list[{self.items.python_type_hint}]"


class AnyOf(TypeBase):
    """
    This pydantic class models the "anyOf" keyword in a json schema validation file.
    """

    any_of: Annotated[list["SchemaType"], Field(alias="anyOf")]

    @property
    def python_type_hint(self) -> str:
        """
        Return the python type hint for this type.
        """
        return " | ".join(sub_field.python_type_hint for sub_field in self.any_of)


class AllOf(TypeBase):
    """
    This pydantic class models the "allOf" keyword in a json schema validation file.
    """

    all_of: Annotated[list["SchemaType"], Field(alias="allOf")]

    @property
    def python_type_hint(self) -> str:
        """
        Unsupported. This features needs a more complex implementation than a simple type hint.
        """
        raise NotImplementedError("Simple python type hint not supported for allOf.")


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

    @property
    def python_type_hint(self) -> str:
        """
        Return the python type hint for this type.
        """
        return "str"


class Number(TypeBase):
    """
    This pydantic class models the "number" type in a json schema validation file.
    """

    type: Literal["number"]

    @property
    def python_type_hint(self) -> str:
        """
        Return the python type hint for this type.
        """
        return "float"


class Decimal(TypeBase):
    """
    This pydantic class models the "decimal" type in a json schema validation file.
    """

    type: Literal["string", "number"]
    format: Literal["decimal"]

    @property
    def python_type_hint(self) -> str:
        """
        Return the python type hint for this type.
        """
        return "Decimal"


class Integer(TypeBase):
    """
    This pydantic class models the "integer" type in a json schema validation file.
    """

    type: Literal["integer"]

    @property
    def python_type_hint(self) -> str:
        """
        Return the python type hint for this type.
        """
        return "int"


class Boolean(TypeBase):
    """
    This pydantic class models the "boolean" type in a json schema validation file.
    """

    type: Literal["boolean"]

    @property
    def python_type_hint(self) -> str:
        """
        Return the python type hint for this type.
        """
        return "bool"


class Null(TypeBase):
    """
    This pydantic class models the "null" type in a json schema validation file.
    """

    type: Literal["null"]

    @property
    def python_type_hint(self) -> str:
        """
        Return the python type hint for this type.
        """
        return "None"


class Any(TypeBase):
    """
    This pydantic class models the "any" type in a json schema validation file.
    """

    @property
    def python_type_hint(self) -> str:
        """
        Return the python type hint for this type.
        """
        return "Any"


REGEX_REL_REFERENCE = re.compile(r"^(?:\.\.[/\\](?:(?:bo|com|enum)[/\\])?|\.[/\\]|[/\\]|)(?P<cls_name>\w+)\.json#?$")
# Here is what will be matched by this regex: https://regex101.com/r/B36QY8/1


class Reference(TypeBase):
    """
    This pydantic class models the "$ref" keyword in a json schema validation file.
    """

    ref: Annotated[str, Field(alias="$ref")]

    @property
    def python_type_hint(self) -> str:
        """
        Return the python type hint for this type.
        """
        match = REGEX_REL_REFERENCE.fullmatch(self.ref)
        if match is None:
            raise ValueError(f"Unsupported reference type to construct python type hint: {self.ref}")
        return match.group("cls_name")


SchemaType = Union[
    Object, StrEnum, Array, AnyOf, AllOf, Decimal, String, Integer, Number, Boolean, Null, Reference, Any
]
SchemaClassType = Union[Object, StrEnum]
SchemaRootType = Union[SchemaRootObject, SchemaRootStrEnum]
SchemaRootTypeBase.model_rebuild()
SchemaRootObject.model_rebuild()
SchemaRootStrEnum.model_rebuild()
