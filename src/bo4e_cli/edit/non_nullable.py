"""
This module contains functions to transform fields which can be null to fields which can't be null.
"""

import re

from more_itertools import first_true

from bo4e_cli.io.console import CONSOLE
from bo4e_cli.models.meta import Schemas
from bo4e_cli.models.schema import AnyOf, Null, SchemaRootObject
from bo4e_cli.utils.fields import get_all_field_paths_from_schema


def field_to_non_nullable(schema_parsed: SchemaRootObject, field_name: str) -> None:
    """
    Convert a field which can be null to a field which can't, by removing the Null type.
    If the field is an "AnyOf" field with only one type left (after removing the Null type), the type is reduced to
    the remaining type - i.e. the structure is flattened.
    If the field has a default value of "null", the default value is removed.
    """
    field_with_null_type = schema_parsed.properties[field_name]
    assert isinstance(
        field_with_null_type, AnyOf
    ), f"Internal error: Expected field to be of type AnyOf but got {type(field_with_null_type)}"
    null_type = first_true(field_with_null_type.any_of, pred=lambda item: isinstance(item, Null), default=None)
    assert null_type is not None, f"Expected {field_with_null_type} to contain Null"
    assert (
        "default" in field_with_null_type.__pydantic_fields_set__
    ), f"Expected {field_with_null_type} to have a default"
    field_with_null_type.any_of.remove(null_type)
    if field_with_null_type.default is None and "default" in field_with_null_type.__pydantic_fields_set__:
        field_with_null_type.__pydantic_fields_set__.remove("default")
        if field_name not in schema_parsed.required:
            schema_parsed.__pydantic_fields_set__.add("required")
            schema_parsed.required.append(field_name)
    if len(field_with_null_type.any_of) == 1:
        # If AnyOf has only one item left, we are reducing the type to that item and copying all relevant data from the
        # AnyOf object
        new_field = field_with_null_type.any_of[0]
        for key in field_with_null_type.__pydantic_fields_set__:
            if hasattr(new_field, key):
                setattr(new_field, key, getattr(field_with_null_type, key))
        schema_parsed.properties[field_name] = new_field


def transform_all_non_nullable_fields(non_nullable_field_patters: list[str], schemas: Schemas) -> None:
    """
    Apply the required field patterns to all schemas.
    """
    field_paths = [
        (field_path, field_name, schema)
        for schema in schemas
        for field_path, field_name in get_all_field_paths_from_schema(schema)
    ]
    for pattern in non_nullable_field_patters:
        compiled_pattern = re.compile(pattern)
        matches = 0
        for field_path, field_name, schema in field_paths:
            schema_parsed = schema.schema_parsed
            if (
                compiled_pattern.fullmatch(field_path)
                and isinstance(schema_parsed, SchemaRootObject)
                and isinstance(schema_parsed.properties[field_name], AnyOf)
                and "default" in schema_parsed.properties[field_name].__pydantic_fields_set__
            ):
                matches += 1
                field_to_non_nullable(schema_parsed, field_name)
                CONSOLE.print(f"Applied pattern '{pattern}' to field {field_path}", show_only_on_verbose=True)
        if matches == 0:
            CONSOLE.print(f"Pattern '{pattern}' did not match any fields", style="warning", show_only_on_verbose=True)
        else:
            CONSOLE.print(f"Pattern '{pattern}' matched {matches} fields", show_only_on_verbose=True)
