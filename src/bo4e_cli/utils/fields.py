"""
Utility functions to work with schema fields.
"""

from collections.abc import Iterator
from typing import Iterable, TypeVar, Union

from mypyc.irbuild.builder import overload
from pydantic import BaseModel

from bo4e_cli.models.meta import SchemaMeta
from bo4e_cli.models.schema import AllOf, AnyOf, Array, Object, Reference, SchemaType


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


T1 = TypeVar("T1", bound=SchemaType)
T2 = TypeVar("T2", bound=SchemaType)
T3 = TypeVar("T3", bound=SchemaType)
T4 = TypeVar("T4", bound=SchemaType)


@overload
def iter_schema_type(schema_type: SchemaType, yield_type_1: type[T1]) -> Iterator[T1]:
    ...


@overload
def iter_schema_type(
    schema_type: SchemaType, yield_type_1: type[T1], yield_type_2: type[T2]
) -> Iterator[Union[T1, T2]]:
    ...


@overload
def iter_schema_type(
    schema_type: SchemaType, yield_type_1: type[T1], yield_type_2: type[T2], yield_type_3: type[T3]
) -> Iterator[Union[T1, T2, T3]]:
    ...


@overload
def iter_schema_type(
    schema_type: SchemaType,
    yield_type_1: type[T1],
    yield_type_2: type[T2],
    yield_type_3: type[T3],
    yield_type_4: type[T4],
) -> Iterator[Union[T1, T2, T3, T4]]:
    ...


@overload
def iter_schema_type(schema_type: SchemaType, *yield_types: type[SchemaType]) -> Iterator[SchemaType]:
    ...


def iter_schema_type(schema_type: SchemaType, *yield_types: type[SchemaType]) -> Iterator[SchemaType]:
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
