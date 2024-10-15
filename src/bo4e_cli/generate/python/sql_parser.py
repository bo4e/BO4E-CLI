"""
Contains code to generate SQLModel classes from json schemas.
Since the used tool doesn't support all features we need, we monkey patch some functions.
"""

import itertools
from pathlib import Path
from typing import Any

from datamodel_code_generator.imports import Import
from jinja2 import Environment, FileSystemLoader
from more_itertools import one
from sqlalchemy import types as sqlalchemy_types

from bo4e_cli.io.schemas import write_schemas
from bo4e_cli.models import schema as schema_models
from bo4e_cli.models.meta import SchemaMeta, Schemas
from bo4e_cli.models.sqlmodel import (
    AdditionalParserKwargs,
    ManyToManyRelationship,
    ManyToManyRelationships,
    SQLModelField,
)
from bo4e_cli.utils.fields import (
    extract_docstring,
    is_enum_reference,
    is_nullable_array,
    is_nullable_field,
    iter_schema_type,
)
from bo4e_cli.utils.imports import relative_import
from bo4e_cli.utils.strings import (
    camel_to_snake,
    construct_id_field_name,
    escaped,
    pydantic_field_name,
    snake_to_pascal,
)

SCHEMA_TYPE_AS_SQLALCHEMY_TYPE: dict[type[schema_models.SchemaType], type[sqlalchemy_types.TypeEngine[Any]]] = {
    schema_models.String: sqlalchemy_types.String,
    schema_models.Integer: sqlalchemy_types.Integer,
    schema_models.Number: sqlalchemy_types.Float,
    schema_models.Boolean: sqlalchemy_types.Boolean,
    schema_models.Decimal: sqlalchemy_types.Numeric,
}


def adapt_parse_for_sql_model(
    input_directory: Path, schemas: Schemas
) -> tuple[AdditionalParserKwargs, Path, ManyToManyRelationships]:
    """Adapt the parser of datamodel-code-generator to suit the needs of SQLModel.

    Scans fields of parsed classes to modify them to meet the SQLModel specifics and to introduce relationships.

    Args:
        input_directory: The directory containing the JSON files with the schemas.
        schemas: The schemas to adapt. Should correspond to the JSON files in the input directory.
    Returns:
        Additional kwargs for the datamodel-code-generator parser, the path to the intermediate JSON files which
         should be parsed instead and the many-to-many relationships.
    """
    additional_parser_kwargs = AdditionalParserKwargs()
    many_to_many_relationships: ManyToManyRelationships = []

    for schema in schemas:
        if schema.module[0] == "enum":
            continue

        assert isinstance(schema.schema_parsed, schema_models.SchemaRootObject)
        del_fields = set()
        # All special cases will be deleted from the schema to prevent datamodel-code-generator from generating them.
        # They will be handled separately by making use of extra_template_data.
        for field_name, field in schema.schema_parsed.properties.items():
            if field_name == "_id":
                add_id_field(schema, additional_parser_kwargs, field)
                del_fields.add(field_name)
                continue
            ref: schema_models.Reference
            match field:
                case schema_models.Any():  # Any field
                    handle_any_field(schema, field, field_name, additional_parser_kwargs)
                case schema_models.Array(items=schema_models.Any()):  # List[Any] field
                    handle_any_field(schema, field.items, field_name, additional_parser_kwargs, is_list=True)
                case schema_models.AnyOf() as field_obj if is_nullable_field(
                    field_obj, schema_models.Any
                ):  # Optional[Any] field
                    handle_any_field(schema, field_obj, field_name, additional_parser_kwargs, is_nullable=True)
                case schema_models.AnyOf() as field_obj if is_nullable_array(
                    field_obj, schema_models.Any
                ):  # Optional[List[Any]] field
                    handle_any_field(
                        schema,
                        one(iter_schema_type(field_obj, schema_models.Array)).items,
                        field_name,
                        additional_parser_kwargs,
                        is_list=True,
                        is_nullable=True,
                    )
                case schema_models.Reference() as field_obj if is_enum_reference(field_obj, schemas):
                    # Reference field referencing an enum
                    ref = field
                    ref_schema = schemas.names[ref.python_type_hint]
                    handle_reference_enum_field(schema, field, ref, ref_schema, field_name, additional_parser_kwargs)
                case schema_models.Array(items=schema_models.Reference()) as field_obj if is_enum_reference(
                    field_obj, schemas
                ):
                    # List[Reference] field containing references to enums
                    ref: schema_models.Reference = field.items  # type: ignore[no-redef]
                    ref_schema = schemas.names[ref.python_type_hint]
                    handle_reference_enum_field(
                        schema, field, ref, ref_schema, field_name, additional_parser_kwargs, is_list=True
                    )
                case schema_models.AnyOf() as field_obj if is_nullable_field(
                    field_obj, schema_models.Reference
                ) and is_enum_reference(field_obj, schemas):
                    # Optional[Reference] field referencing an enum
                    ref = one(
                        sub_field for sub_field in field_obj.any_of if isinstance(sub_field, schema_models.Reference)
                    )
                    ref_schema = schemas.names[ref.python_type_hint]
                    handle_reference_enum_field(
                        schema, field_obj, ref, ref_schema, field_name, additional_parser_kwargs, is_nullable=True
                    )
                case schema_models.AnyOf() as field_obj if is_nullable_array(
                    field_obj, schema_models.Reference
                ) and is_enum_reference(field_obj, schemas):
                    # Optional[List[Reference]] field containing references to enums
                    ref = one(  # type: ignore[assignment]
                        # type of sub_field.items is checked to be Reference in the is_enum_reference function
                        sub_field.items
                        for sub_field in field_obj.any_of
                        if isinstance(sub_field, schema_models.Array)
                    )
                    ref_schema = schemas.names[ref.python_type_hint]
                    handle_reference_enum_field(
                        schema,
                        field_obj,
                        ref,
                        ref_schema,
                        field_name,
                        additional_parser_kwargs,
                        is_list=True,
                        is_nullable=True,
                    )
                case schema_models.Reference():
                    # Reference field
                    ref = field
                    ref_schema = schemas.names[ref.python_type_hint]
                    handle_reference_field(schema, field, ref, ref_schema, field_name, additional_parser_kwargs)
                case schema_models.Array(items=schema_models.Reference()):
                    # List[Reference] field containing references
                    ref = field.items  # type: ignore[assignment]
                    # type of sub_field.items is checked to be Reference by the structural pattern matching
                    ref_schema = schemas.names[ref.python_type_hint]
                    handle_reference_list_field(
                        schema,
                        field,
                        ref,
                        ref_schema,
                        field_name,
                        additional_parser_kwargs,
                        many_to_many_relationships,
                    )
                case schema_models.AnyOf() as field_obj if is_nullable_field(field_obj, schema_models.Reference):
                    # Optional[Reference] field
                    ref = one(
                        sub_field for sub_field in field_obj.any_of if isinstance(sub_field, schema_models.Reference)
                    )
                    ref_schema = schemas.names[ref.python_type_hint]
                    handle_reference_field(
                        schema, field_obj, ref, ref_schema, field_name, additional_parser_kwargs, is_nullable=True
                    )
                case schema_models.AnyOf() as field_obj if is_nullable_array(field_obj, schema_models.Reference):
                    # Optional[List[Reference]] field containing references
                    ref = one(  # type: ignore[assignment]
                        # type of sub_field.items is checked to be Reference in the is_nullable_array function
                        sub_field.items
                        for sub_field in field_obj.any_of
                        if isinstance(sub_field, schema_models.Array)
                    )
                    ref_schema = schemas.names[ref.python_type_hint]
                    handle_reference_list_field(
                        schema,
                        field_obj,
                        ref,
                        ref_schema,
                        field_name,
                        additional_parser_kwargs,
                        many_to_many_relationships,
                        is_nullable=True,
                    )
                case schema_models.Array():
                    # List field without containing Reference or Any fields
                    handle_array_field(
                        schema,
                        field,
                        one(iter_schema_type(field, schema_models.Array)),
                        field_name,
                        additional_parser_kwargs,
                    )
                case schema_models.AnyOf() as field_obj if is_nullable_field(field_obj, schema_models.Array):
                    # Optional[List] field without containing Reference or Any fields
                    handle_array_field(
                        schema,
                        field,
                        one(iter_schema_type(field, schema_models.Array)),
                        field_name,
                        additional_parser_kwargs,
                        is_nullable=True,
                    )
                case _:
                    continue
                    # 'cause everything else should be handled well by datamodel-code-generator
            del_fields.add(field_name)

        for field_name in del_fields:
            del schema.schema_parsed.properties[field_name]

        if "_id" not in del_fields:
            add_id_field(
                schema, additional_parser_kwargs, schema_models.String.model_construct(title="Primary key ID-Field")
            )

    # parsed_arguments = additional_parser_kwargs.model_dump(mode="python")
    tmp_path = input_directory / "intermediate"
    write_schemas(schemas, tmp_path, include_version_file=False, enable_tracker=False)
    return additional_parser_kwargs, tmp_path, many_to_many_relationships


def add_id_field(
    schema: SchemaMeta, additional_parser_kwargs: AdditionalParserKwargs, id_field: schema_models.SchemaType
) -> None:
    """Add an id field to the schema.

    Args:
        schema: The schema to add the id field to
        additional_parser_kwargs: The additional parser kwargs
        id_field: The field object to use as the id field
    """
    additional_parser_kwargs.extra_template_data[schema.name].sql.fields["id"] = SQLModelField(
        name="id",
        annotation="uuid_pkg.UUID",
        definition=f'Field(default_factory=uuid_pkg.uuid4, primary_key=True, alias="_id", title="{id_field.title}")',
        description="The primary key of the table as a UUID4.",
    )


def build_field_definition(
    field_name: str,
    field: schema_models.SchemaType | None,
    **options: str,
) -> str:
    """Build the field definition for a SQLModel field.

    If not overridden by the options, the default value, title and alias are added to the field definition using the
    field name and the field object. If the field object is None, the default value and the title will not be loaded.

    Args:
        field_name: The name of the field
        field: The field object
        options: Additional options for the field definition
    """
    if "default" not in options and field is not None:
        options["default"] = "..." if "default" not in field.model_fields_set else repr(field.default)
    if "title" not in options and field is not None and "title" in field.model_fields_set:
        options["title"] = escaped(field.title)
    if "alias" not in options:
        pyd_field_name, alias = pydantic_field_name(field_name)
        if pyd_field_name != alias:
            options["alias"] = escaped(alias)

    items = itertools.chain(
        [(key, options.pop(key)) for key in ("default", "title", "alias") if key in options],
        sorted(options.items()),
    )

    field_def = "Field("
    field_def += ", ".join(f"{k}={v}" for k, v in items if v is not None)
    field_def += ")"
    return field_def


# pylint: disable=too-many-arguments,too-many-positional-arguments
def handle_any_field(
    schema: SchemaMeta,
    field: schema_models.SchemaType,
    field_name: str,
    additional_parser_kwargs: AdditionalParserKwargs,
    is_list: bool = False,
    is_nullable: bool = False,
) -> None:
    """
    Handle the case where a field is of type Any.
    """
    pyd_field_name, _ = pydantic_field_name(field_name)
    if is_list:
        field_definition = build_field_definition(
            field_name,
            field,
            sa_column=f"Column(ARRAY(PickleType), nullable={is_nullable})",
        )
        additional_parser_kwargs.extra_template_data[schema.name].sql.imports.add(
            Import.from_full_path("sqlalchemy.ARRAY")
        )
    else:
        field_definition = build_field_definition(
            field_name, field, sa_column=f"Column(PickleType, nullable={is_nullable})"
        )
    additional_parser_kwargs.extra_template_data[schema.name].sql.imports.update(
        [
            Import.from_full_path("typing.Any"),
            Import.from_full_path("sqlalchemy.Column"),
            Import.from_full_path("sqlalchemy.PickleType"),
        ]
    )
    additional_parser_kwargs.extra_template_data[schema.name].sql.fields[pyd_field_name] = SQLModelField(
        name=pyd_field_name,
        annotation=field.python_type_hint,
        definition=field_definition,
        description=extract_docstring(field),
    )


def handle_reference_field(
    schema: SchemaMeta,
    field: schema_models.SchemaType,
    reference: schema_models.Reference,
    referenced_schema: SchemaMeta,
    field_name: str,
    additional_parser_kwargs: AdditionalParserKwargs,
    is_nullable: bool = False,
) -> None:
    """
    Handle the case where a field is of type Reference or Optional[Reference].
    """
    default_value = "..." if "default" not in field.model_fields_set else str(field.default)
    assert default_value in ("None", "..."), f"Unexpected default value {default_value}"
    pyd_field_name, _ = pydantic_field_name(field_name)
    field_name_id = construct_id_field_name(pyd_field_name)
    annotation_field_name_id = "uuid_pkg.UUID"
    annotation_field_name = reference.python_type_hint
    reference_name = reference.python_type_hint
    reference_table_name = reference_name.lower()
    if is_nullable:
        annotation_field_name_id += " | None"
        annotation_field_name += " | None"
        field_id_definition = build_field_definition(
            field_name_id, field, foreign_key=escaped(f"{reference_table_name}.id"), ondelete=escaped("SET NULL")
        )
    else:
        assert default_value == "...", f"Unexpected default value {default_value}"
        field_id_definition = build_field_definition(
            field_name_id, field, foreign_key=escaped(f"{reference_table_name}.id")
        )

    additional_parser_kwargs.extra_template_data[schema.name].sql.imports.update(
        [
            Import.from_full_path("sqlmodel.Relationship"),
            relative_import(schema.python_module_path, referenced_schema.python_class_path),
        ]
    )
    additional_parser_kwargs.extra_template_data[schema.name].sql.fields[field_name_id] = SQLModelField(
        name=field_name_id,
        annotation=annotation_field_name_id,
        definition=field_id_definition,
        description=f"The id to implement the relationship (field {pyd_field_name} references {reference_name}).",
    )
    additional_parser_kwargs.extra_template_data[schema.name].sql.fields[pyd_field_name] = SQLModelField(
        name=pyd_field_name,
        annotation=annotation_field_name,
        definition=f'Relationship(sa_relationship_kwargs={{"foreign_keys": ["{schema.name}.{field_name_id}"]}})',
        description=extract_docstring(field),
    )


def handle_reference_list_field(
    schema: SchemaMeta,
    field: schema_models.SchemaType,
    reference: schema_models.Reference,
    referenced_schema: SchemaMeta,
    field_name: str,
    additional_parser_kwargs: AdditionalParserKwargs,
    many_to_many_relationships: list[ManyToManyRelationship],
    is_nullable: bool = False,
) -> None:
    """
    Handle the case where a field is of type List[Reference] or Optional[List[Reference]].
    """
    default_value = "..." if "default" not in field.model_fields_set else str(field.default)
    assert default_value in ("None", "..."), f"Unexpected default value {default_value}"
    pyd_field_name, _ = pydantic_field_name(field_name)
    annotation_field_name = f"list[{reference.python_type_hint}]"
    reference_name = reference.python_type_hint
    link_table_name = f"{schema.name}{snake_to_pascal(pyd_field_name)}Link"
    if is_nullable:
        annotation_field_name += " | None"
    else:
        assert default_value == "...", f"Unexpected default value {default_value}"

    additional_parser_kwargs.extra_template_data[schema.name].sql.imports.update(
        [
            Import.from_full_path("sqlmodel.Relationship"),
            Import.from_full_path(f"..many.{link_table_name}"),
            relative_import(schema.python_module_path, referenced_schema.python_class_path),
        ]
    )
    additional_parser_kwargs.extra_template_data[schema.name].sql.fields[pyd_field_name] = SQLModelField(
        name=pyd_field_name,
        annotation=annotation_field_name,
        definition=f"Relationship(link_model={link_table_name})",
        description=extract_docstring(field),
    )
    many_to_many_relationships.append(
        ManyToManyRelationship(
            table_name=link_table_name,
            cls1=schema.name,
            cls2=reference_name,
            rel_field_name1=pyd_field_name,
            rel_field_name2=None,
            id_field_name1=f"{camel_to_snake(schema.name)}_id",
            id_field_name2=f"{camel_to_snake(reference_name)}_id",
        )
    )


def handle_reference_enum_field(
    schema: SchemaMeta,
    field: schema_models.SchemaType,
    reference: schema_models.Reference,
    referenced_schema: SchemaMeta,
    field_name: str,
    additional_parser_kwargs: AdditionalParserKwargs,
    is_nullable: bool = False,
    is_list: bool = False,
) -> None:
    """
    Handle the case where a field is of type Reference or Optional[Reference] and references an enum.
    """
    reference_name = reference.python_type_hint
    if "default" in field.model_fields_set and field.default is not None:
        default_value = f"{reference_name}.{field.default}"
    elif "default" in field.model_fields_set:
        default_value = str(field.default)
        assert default_value == "None" and is_nullable, f"Unexpected default value {default_value}"
    else:
        default_value = "..."
    pyd_field_name, _ = pydantic_field_name(field_name)
    annotation_field_name = reference_name
    if is_list:
        field_definition = build_field_definition(
            field_name,
            field,
            default=default_value,
            sa_column=f'Column(ARRAY(Enum({reference_name}, name="{reference_name.lower()}")))',
        )
        annotation_field_name = f"list[{annotation_field_name}]"
        additional_parser_kwargs.extra_template_data[schema.name].sql.imports.update(
            [
                Import.from_full_path("sqlalchemy.ARRAY"),
                Import.from_full_path("sqlalchemy.Enum"),
                Import.from_full_path("sqlalchemy.Column"),
            ]
        )
    else:
        field_definition = build_field_definition(field_name, field, default=default_value)
    if is_nullable:
        annotation_field_name += " | None"

    additional_parser_kwargs.extra_template_data[schema.name].sql.imports.add(
        relative_import(schema.python_module_path, referenced_schema.python_class_path)
    )
    additional_parser_kwargs.extra_template_data[schema.name].sql.fields[pyd_field_name] = SQLModelField(
        name=pyd_field_name,
        annotation=annotation_field_name,
        definition=field_definition,
        description=extract_docstring(field),
    )


def handle_array_field(
    schema: SchemaMeta,
    field: schema_models.SchemaType,
    array_field: schema_models.Array,
    field_name: str,
    additional_parser_kwargs: AdditionalParserKwargs,
    is_nullable: bool = False,
) -> None:
    """
    Handle the case where a field is of type List or Optional[List].
    """
    annotation_field_name = f"list[{array_field.items.python_type_hint}]"
    if is_nullable:
        annotation_field_name += " | None"
    if isinstance(array_field.items, schema_models.Decimal):
        additional_parser_kwargs.extra_template_data[schema.name].sql.imports.add(
            Import.from_full_path("decimal.Decimal")
        )
    sa_type = SCHEMA_TYPE_AS_SQLALCHEMY_TYPE.get(type(array_field.items))
    if sa_type is None:
        raise ValueError(f"Unsupported type inside array: {array_field.items}")

    additional_parser_kwargs.extra_template_data[schema.name].sql.imports.update(
        [
            Import.from_full_path("sqlalchemy.Column"),
            Import.from_full_path("sqlalchemy.ARRAY"),
            Import.from_full_path(f"sqlalchemy.{sa_type.__name__}"),
        ]
    )
    additional_parser_kwargs.extra_template_data[schema.name].sql.fields[field_name] = SQLModelField(
        name=field_name,
        annotation=annotation_field_name,
        definition=build_field_definition(field_name, field, sa_column=f"Column(ARRAY({sa_type.__name__}))"),
        description=extract_docstring(field),
    )


def parse_many_many_links(links: ManyToManyRelationships, custom_template_dir: Path) -> str:
    """
    use template to write many-to-many link classes to many.py file
    """
    environment = Environment(loader=FileSystemLoader(custom_template_dir))
    template = environment.get_template("ManyLinks.jinja2")
    python_code = template.render({"links": links})
    return python_code
