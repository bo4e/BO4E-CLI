"""
Contains utility functions used mainly in the Python code generation.
"""
from typing import Optional, TypeVar

from datamodel_code_generator.imports import Import
from datamodel_code_generator.parser.base import relative
from more_itertools import one

from bo4e_cli.models.meta import Schemas
from bo4e_cli.models.schema import AnyOf, Array, Null, Reference, SchemaType
from bo4e_cli.utils.strings import camel_to_snake


def pydantic_field_name(field_name: str) -> str:
    return camel_to_snake(field_name.lstrip("_"))


def construct_id_field_name(relationship_field_name: str) -> str:
    return f"{relationship_field_name}_id"


def extract_docstring(field: SchemaType) -> Optional[str]:
    return field.description if "description" in field.model_fields_set else None


def is_optional(field: AnyOf, other_type: Optional[type[SchemaType]] = None) -> bool:
    if len(field.any_of) == 2:
        if any(isinstance(sub_field, Null) for sub_field in field.any_of):
            return other_type is None or any(isinstance(sub_field, other_type) for sub_field in field.any_of)
    return False


def is_optional_array(field: AnyOf, other_type: type[SchemaType]) -> bool:
    if len(field.any_of) == 2:
        if not any(isinstance(sub_field, Null) for sub_field in field.any_of):
            return False
        return any(
            isinstance(sub_field, Array) and isinstance(sub_field.items, other_type) for sub_field in field.any_of
        )
    return False


def is_enum_reference(field: Reference | AnyOf | Array, schemas: Schemas) -> bool:
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


T = TypeVar("T", bound=SchemaType)


def is_optional_or_same(field: AnyOf | T, other_type: type[T]) -> bool:
    if isinstance(field, other_type):
        return True
    if not isinstance(field, AnyOf):
        return False
    if len(field.any_of) == 2:
        if any(isinstance(sub_field, Null) for sub_field in field.any_of):
            return any(isinstance(sub_field, other_type) for sub_field in field.any_of)
    return False


def relative_import(cur_module: str, reference: str) -> Import:
    from_, import_ = relative(cur_module, reference)
    return Import(from_=from_, import_=import_)
