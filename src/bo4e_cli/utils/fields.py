"""
Utility functions to work with schema fields.
"""

from collections.abc import Iterator
from typing import Iterable, Optional, TypeVar, Union, overload

from more_itertools import one
from pydantic import BaseModel

from bo4e_cli.models.meta import SchemaMeta, Schemas
from bo4e_cli.models.schema import AllOf, AnyOf, Array, Null, Object, Reference, SchemaType


def get_all_field_paths_from_schema(schema: SchemaMeta) -> Iterable[tuple[str, str]]:
    """
    Get all field paths of the schema.
    Returns an iterable of tuples with the field path and the field name.
    An element could be e.g. ("bo.Angebot.angebotsnehmer", "angebotsnehmer").
    """
    if not isinstance(schema.schema_parsed, Object):
        return
    for field_name in schema.schema_parsed.properties:
        yield ".".join((*schema.module, field_name)), field_name


def is_unset(model: BaseModel, field_name: str) -> bool:
    """
    Check if a field is unset in a pydantic model.
    """
    return field_name not in model.model_fields_set


# pylint: disable=invalid-name
# Apparently, pylint doesn't support numbers in its regex. To play around with the regex:
# https://regex101.com/r/1Hja8E/1
T1 = TypeVar("T1", bound=SchemaType)
T2 = TypeVar("T2", bound=SchemaType)
T3 = TypeVar("T3", bound=SchemaType)
T4 = TypeVar("T4", bound=SchemaType)


@overload
def iter_schema_type(schema_type: SchemaType, yield_type_1: type[T1]) -> Iterator[T1]: ...


@overload
def iter_schema_type(
    schema_type: SchemaType, yield_type_1: type[T1], yield_type_2: type[T2]
) -> Iterator[Union[T1, T2]]: ...


@overload
def iter_schema_type(
    schema_type: SchemaType, yield_type_1: type[T1], yield_type_2: type[T2], yield_type_3: type[T3]
) -> Iterator[Union[T1, T2, T3]]: ...


@overload
def iter_schema_type(
    schema_type: SchemaType,
    yield_type_1: type[T1],
    yield_type_2: type[T2],
    yield_type_3: type[T3],
    yield_type_4: type[T4],
) -> Iterator[Union[T1, T2, T3, T4]]: ...


@overload
def iter_schema_type(schema_type: SchemaType, *yield_types: type[SchemaType]) -> Iterator[SchemaType]: ...


def iter_schema_type(  # type: ignore[misc]
    # Overloaded function implementation does not accept all possible arguments of signature 1
    # mypy doesn't understand that the *yield_types argument covers all overloaded signatures
    schema_type: SchemaType,
    *yield_types: type[SchemaType],
) -> Iterator[SchemaType]:
    """
    Iterate recursively through the schema type. Yields all objects of the given types.
    """

    def iter_base(_object: SchemaType) -> Iterator[SchemaType]:
        if isinstance(_object, yield_types):
            yield _object
        if isinstance(_object, Object):
            yield from iter_iter(_object.properties.values())
        elif isinstance(_object, AnyOf):
            yield from iter_iter(_object.any_of)
        elif isinstance(_object, AllOf):
            yield from iter_iter(_object.all_of)
        elif isinstance(_object, Array):
            yield from iter_base(_object.items)

    def iter_iter(iterator: Iterable[SchemaType]) -> Iterator[SchemaType]:
        for item in iterator:
            yield from iter_base(item)

    yield from iter_base(schema_type)


def extract_docstring(field: SchemaType) -> Optional[str]:
    """
    Extract the docstring from a SchemaType. Returns None, if no docstring was defined.
    """
    return field.description if "description" in field.model_fields_set else None


def is_nullable_field(field: AnyOf, other_type: Optional[type[SchemaType]] = None) -> bool:
    """
    Check if a AnyOf field represents a nullable field. If other_type is provided, check if the field is nullable
    and of the given type.
    """
    if len(field.any_of) == 2:
        if any(isinstance(sub_field, Null) for sub_field in field.any_of):
            return other_type is None or any(isinstance(sub_field, other_type) for sub_field in field.any_of)
    return False


def is_nullable_array(field: AnyOf, other_type: type[SchemaType]) -> bool:
    """
    Check if a AnyOf field represents a nullable array. If other_type is provided, check if the field is a nullable
    array and of the given type.
    """
    if len(field.any_of) == 2:
        if not any(isinstance(sub_field, Null) for sub_field in field.any_of):
            return False
        return any(
            isinstance(sub_field, Array) and isinstance(sub_field.items, other_type) for sub_field in field.any_of
        )
    return False


def is_enum_reference(field: Reference | AnyOf | Array, schemas: Schemas) -> bool:
    """
    Check if a field contains an enum reference.
    """
    if isinstance(field, Array):
        assert isinstance(field.items, Reference), "Internal error: Array.items should be a Reference"
        field = field.items
    if isinstance(field, Reference):
        assert (
            field.python_type_hint in schemas.names
        ), "Internal error: Reference.python_type_hint should be in schemas"
        return schemas.names[field.python_type_hint].module[0] == "enum"
    if isinstance(field, AnyOf):
        ref = one(sub_field for sub_field in field.any_of if isinstance(sub_field, (Reference, Array)))
        return is_enum_reference(ref, schemas)
    raise ValueError(f"Unexpected field type {type(field)}")
