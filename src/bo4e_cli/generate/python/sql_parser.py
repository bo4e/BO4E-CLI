from pathlib import Path

from datamodel_code_generator.imports import Import
from jinja2 import Environment, FileSystemLoader
from more_itertools import one
from sqlalchemy import types as sqlalchemy_types

from bo4e_cli.generate.python.utils import (
    construct_id_field_name,
    extract_docstring,
    is_enum_reference,
    is_optional,
    is_optional_array,
    pydantic_field_name,
    relative_import,
)
from bo4e_cli.io.schemas import write_schemas
from bo4e_cli.models import schema as schema_models
from bo4e_cli.models.meta import SchemaMeta, Schemas
from bo4e_cli.models.sqlmodel import (
    AdditionalParserKwargs,
    ManyToManyRelationship,
    ManyToManyRelationships,
    SQLModelField,
)
from bo4e_cli.utils.fields import iter_schema_type
from bo4e_cli.utils.strings import camel_to_snake, snake_to_pascal

SCHEMA_TYPE_AS_SQLALCHEMY_TYPE: dict[type[schema_models.SchemaType], type[sqlalchemy_types.TypeEngine]] = {
    schema_models.String: sqlalchemy_types.String,
    schema_models.Integer: sqlalchemy_types.Integer,
    schema_models.Number: sqlalchemy_types.Float,
    schema_models.Boolean: sqlalchemy_types.Boolean,
    schema_models.Decimal: sqlalchemy_types.Numeric,
}


# TODO: Set titles of Fields


def adapt_parse_for_sql_model(
    input_directory: Path, schemas: Schemas
) -> tuple[Schemas, AdditionalParserKwargs, Path, ManyToManyRelationships]:
    """
    Scans fields of parsed classes to modify them to meet the SQLModel specifics and to introduce relationships.
    Returns additional information, an input path with modified json schemas and arguments for the parser
    """
    additional_parser_kwargs = AdditionalParserKwargs()
    many_to_many_relationships: ManyToManyRelationships = []

    for schema in schemas:
        if schema.module[0] == "enum":
            continue

        del_fields = set()
        # All special cases will be deleted from the schema to prevent datamodel-code-generator from generating them.
        # They will be handled separately by making use of extra_template_data.
        for field_name, field in schema.schema_parsed.properties.items():
            if field_name == "_id":
                add_id_field(schema, additional_parser_kwargs, field)
                del_fields.add(field_name)
                continue
            match field:
                case schema_models.Any():  # Any field
                    handle_any_field(schema, field, field_name, additional_parser_kwargs)
                case schema_models.Array(items=schema_models.Any()):  # List[Any] field
                    handle_any_field(schema, field.items, field_name, additional_parser_kwargs, is_list=True)
                case schema_models.AnyOf() as field_obj if is_optional(
                    field_obj, schema_models.Any
                ):  # Optional[Any] field
                    handle_any_field(schema, field_obj, field_name, additional_parser_kwargs, is_nullable=True)
                case schema_models.AnyOf() as field_obj if is_optional_array(
                    field_obj, schema_models.Any
                ):  # Optional[List[Any]] field
                    handle_any_field(
                        schema, field_obj.items, field_name, additional_parser_kwargs, is_list=True, is_nullable=True
                    )
                case schema_models.Reference() as field_obj if is_enum_reference(field_obj, schemas):
                    # Reference field referencing an enum
                    ref: schema_models.Reference = field
                    ref_schema = schemas.names[ref.python_type_hint]
                    handle_reference_enum_field(schema, field, ref, ref_schema, field_name, additional_parser_kwargs)
                case schema_models.Array(items=schema_models.Reference()) as field_obj if is_enum_reference(
                    field_obj, schemas
                ):
                    # List[Reference] field containing references to enums
                    ref: schema_models.Reference = field.items
                    ref_schema = schemas.names[ref.python_type_hint]
                    handle_reference_enum_field(
                        schema, field, ref, ref_schema, field_name, additional_parser_kwargs, is_list=True
                    )
                case schema_models.AnyOf() as field_obj if is_optional(
                    field_obj, schema_models.Reference
                ) and is_enum_reference(field_obj, schemas):
                    # Optional[Reference] field referencing an enum
                    ref: schema_models.Reference = one(
                        sub_field for sub_field in field_obj.any_of if isinstance(sub_field, schema_models.Reference)
                    )
                    ref_schema = schemas.names[ref.python_type_hint]
                    handle_reference_enum_field(
                        schema, field_obj, ref, ref_schema, field_name, additional_parser_kwargs, is_nullable=True
                    )
                case schema_models.AnyOf() as field_obj if is_optional_array(
                    field_obj, schema_models.Reference
                ) and is_enum_reference(field_obj, schemas):
                    # Optional[List[Reference]] field containing references to enums
                    ref: schema_models.Reference = one(
                        sub_field.items for sub_field in field_obj.any_of if isinstance(sub_field, schema_models.Array)
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
                    ref: schema_models.Reference = field
                    ref_schema = schemas.names[ref.python_type_hint]
                    handle_reference_field(schema, field, ref, ref_schema, field_name, additional_parser_kwargs)
                case schema_models.Array(items=schema_models.Reference()):
                    # List[Reference] field containing references
                    ref: schema_models.Reference = field.items
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
                case schema_models.AnyOf() as field_obj if is_optional(field_obj, schema_models.Reference):
                    # Optional[Reference] field
                    ref: schema_models.Reference = one(
                        sub_field for sub_field in field_obj.any_of if isinstance(sub_field, schema_models.Reference)
                    )
                    ref_schema = schemas.names[ref.python_type_hint]
                    handle_reference_field(
                        schema, field_obj, ref, ref_schema, field_name, additional_parser_kwargs, is_nullable=True
                    )
                case schema_models.AnyOf() as field_obj if is_optional_array(field_obj, schema_models.Reference):
                    # Optional[List[Reference]] field containing references
                    ref: schema_models.Reference = one(
                        sub_field.items for sub_field in field_obj.any_of if isinstance(sub_field, schema_models.Array)
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
                case schema_models.AnyOf() as field_obj if is_optional(field_obj, schema_models.Array):
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

    # parsed_arguments = additional_parser_kwargs.model_dump(mode="python")
    tmp_path = input_directory / "intermediate"
    write_schemas(schemas, tmp_path, include_version_file=False, enable_tracker=False)
    return schemas, additional_parser_kwargs, tmp_path, many_to_many_relationships


def add_id_field(
    schema: SchemaMeta, additional_parser_kwargs: AdditionalParserKwargs, id_field: schema_models.SchemaType
) -> None:
    """
    Add an id field to the schema.
    """
    additional_parser_kwargs.extra_template_data[schema.name].sql.fields["id"] = SQLModelField(
        name="id",
        annotation="uuid_pkg.UUID",
        definition=f'Field(default_factory=uuid_pkg.uuid4, primary_key=True, alias="_id", title="{id_field.title}")',
        description="The primary key of the table as a UUID4.",
    )


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
    default_value = "..." if "default" not in field.model_fields_set else str(field.default)
    field_name = pydantic_field_name(field_name)
    if is_list:
        field_definition = (
            f"Field(default={default_value}, sa_column=Column(ARRAY(PickleType), nullable={is_nullable}))"
        )
        additional_parser_kwargs.extra_template_data[schema.name].sql.imports.add(
            Import.from_full_path("sqlalchemy.ARRAY")
        )
    else:
        field_definition = f"Field(default={default_value}, sa_column=Column(PickleType, nullable={is_nullable}))"
    additional_parser_kwargs.extra_template_data[schema.name].sql.imports.update(
        [
            Import.from_full_path("typing.Any"),
            Import.from_full_path("sqlalchemy.Column"),
            Import.from_full_path("sqlalchemy.PickleType"),
        ]
    )
    additional_parser_kwargs.extra_template_data[schema.name].sql.fields[field_name] = SQLModelField(
        name=field_name,
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
    field_name = pydantic_field_name(field_name)
    field_name_id = construct_id_field_name(field_name)
    annotation_field_name_id = "uuid_pkg.UUID"
    annotation_field_name = reference.python_type_hint
    reference_name = reference.python_type_hint
    reference_table_name = reference_name.lower()
    if is_nullable:
        annotation_field_name_id += " | None"
        annotation_field_name += " | None"
        field_id_definition = (
            f'Field(default={default_value}, foreign_key="{reference_table_name}.id", ondelete="SET NULL")'
        )
    else:
        assert default_value == "...", f"Unexpected default value {default_value}"
        field_id_definition = f'Field(default={default_value}, foreign_key="{reference_table_name}.id")'

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
        description=f"The id to implement the relationship (field {field_name} references {reference_name}).",
    )
    additional_parser_kwargs.extra_template_data[schema.name].sql.fields[field_name] = SQLModelField(
        name=field_name,
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
    field_name = pydantic_field_name(field_name)
    annotation_field_name = f"list[{reference.python_type_hint}]"
    reference_name = reference.python_type_hint
    link_table_name = f"{schema.name}{snake_to_pascal(field_name)}Link"
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
    additional_parser_kwargs.extra_template_data[schema.name].sql.fields[field_name] = SQLModelField(
        name=field_name,
        annotation=annotation_field_name,
        definition=f"Relationship(link_model={link_table_name})",
        description=extract_docstring(field),
    )
    many_to_many_relationships.append(
        ManyToManyRelationship(
            table_name=link_table_name,
            cls1=schema.name,
            cls2=reference_name,
            rel_field_name1=field_name,
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
    field_name = pydantic_field_name(field_name)
    annotation_field_name = reference_name
    if is_list:
        field_definition = f'Field(default={default_value}, sa_column=Column(ARRAY(Enum({reference_name}, name="{reference_name.lower()}"))))'
        annotation_field_name = f"list[{annotation_field_name}]"
        additional_parser_kwargs.extra_template_data[schema.name].sql.imports.update(
            [
                Import.from_full_path("sqlalchemy.ARRAY"),
                Import.from_full_path("sqlalchemy.Enum"),
                Import.from_full_path("sqlalchemy.Column"),
            ]
        )
    else:
        field_definition = f"Field(default={default_value})"
    if is_nullable:
        annotation_field_name += " | None"

    additional_parser_kwargs.extra_template_data[schema.name].sql.imports.add(
        relative_import(schema.python_module_path, referenced_schema.python_class_path)
    )
    additional_parser_kwargs.extra_template_data[schema.name].sql.fields[field_name] = SQLModelField(
        name=field_name,
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
    default_value = "..." if "default" not in field.model_fields_set else str(field.default)
    assert default_value in ("None", "..."), f"Unexpected default value {default_value}"
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
        definition=f"Field(default={default_value}, sa_column=Column(ARRAY({sa_type.__name__})))",
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
