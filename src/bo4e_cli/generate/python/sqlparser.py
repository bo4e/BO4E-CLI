"""
Contains code to generate SQLModel classes from json schemas.
Since the used tool doesn't support all features we need, we monkey patch some functions.
"""

import json
import os
import re
import subprocess
import tempfile
from collections import defaultdict
from pathlib import Path
from typing import Any, DefaultDict, Union

import black
import isort
from jinja2 import Environment, FileSystemLoader

from bo4e_cli.generate.python.schema import SchemaMetadata, camel_to_snake


def remove_pydantic_field_import(python_code: str) -> str:
    """
    Remove the future import from the generated code.
    If sql_model adapt SQLModel specific imports
    """
    # remove Field from pydantic imports
    python_code = re.sub(r"from pydantic import (.*?)Field(.*?)\n", r"from pydantic import \1\2\n", python_code)
    # clean up imports after removing Field, e.g. from pydantic import Something, \n -> from pydantic import Something\n
    python_code = python_code.replace(",,", "").replace(", \n", "\n").replace(" , ", " ")
    python_code = python_code.replace("from pydantic import \n", "")
    return python_code


def adapt_parse_for_sql(
    input_directory: Path, namespace: dict[str, SchemaMetadata]
) -> tuple[dict[str, SchemaMetadata], dict[str, Any], Path, dict[str, str]]:
    """
    Scans fields of parsed classes to modify them to meet the SQLModel specifics and to introduce relationships.
    Returns additional information, an input path with modified json schemas and arguments for the parser
    """
    add_relation: DefaultDict[str, dict[str, Any]] = defaultdict(dict)  # added relationship fields
    relation_imports: DefaultDict[str, dict[str, str]] = defaultdict(dict)  # added imports for relationship fields

    for schema_metadata in namespace.values():
        if schema_metadata.module_path[0] != "enum":
            # list of fields which will be replaced by modified versions
            del_fields = set()
            for field, val in schema_metadata.schema_parsed["properties"].items():
                # type Any field
                if "type" not in str(val):
                    add_relation, relation_imports = create_sql_any(
                        field, schema_metadata.class_name, namespace, add_relation, relation_imports
                    )
                    del_fields.add(field)
                # modify decimal fields
                if "number" in str(val) and "string" in str(val):
                    relation_imports[schema_metadata.class_name + "ADD"]["Decimal"] = "decimal"
                if "array" in str(val) and "$ref" not in str(val):
                    add_relation, relation_imports = create_sql_list(
                        field, schema_metadata.class_name, namespace, add_relation, relation_imports
                    )
                    del_fields.add(field)
                if "$ref" in str(val):  # or "array" in str(val):
                    add_relation, relation_imports = create_sql_field(
                        field, schema_metadata.class_name, namespace, add_relation, relation_imports
                    )
                    del_fields.add(field)
            for field in del_fields:
                del schema_metadata.schema_parsed["properties"][field]
            # delete id field as it is replaced below
            if schema_metadata.schema_parsed["properties"].get("_id"):
                del schema_metadata.schema_parsed["properties"]["_id"]
            # store the reduced version. The modified fields will be added in the BaseModel.jinja2 schema
            schema_metadata.schema_text = json.dumps(schema_metadata.schema_parsed, indent=2, ensure_ascii=False)

    additional_arguments = additional_sql_arguments(namespace, add_relation, relation_imports)
    # save intermediate jsons
    for schema in namespace.values():
        file_path = (
            input_directory / "intermediate" / Path(*schema.module_path[:-1]) / (schema.module_path[-1] + ".json")
        )
        file_path.parent.mkdir(parents=True, exist_ok=True)
        file_path.write_text(schema.schema_text, encoding="utf-8")
    input_directory = input_directory / Path("intermediate")

    links: dict[str, str] = {}
    # write linking classes for SQLModel
    if "MANY" in add_relation:
        links = add_relation["MANY"]

    return namespace, additional_arguments, input_directory, links


def additional_sql_arguments(
    namespace: dict[str, SchemaMetadata],
    add_relation: DefaultDict[str, dict[str, Any]],
    relation_imports: DefaultDict[str, dict[str, str]],
) -> dict[str, Any]:
    """
    returns addition and sql specific arguments to be processed by the standard code generator parser
    """
    additional_arguments: dict[str, Any] = {}  # additional arguments passed to the parser
    additional_sql_data: DefaultDict[str, Any] = defaultdict(
        dict
    )  # additional fields for code generation using templates

    # pass additional fields and imports to dictionary for parser
    for schema_metadata in namespace.values():
        if schema_metadata.module_path[0] != "enum":
            # add primary key
            additional_sql_data[schema_metadata.class_name]["SQL"] = {
                "primary": "id: uuid_pkg.UUID = Field( default_factory=uuid_pkg.uuid4, primary_key=True, index=True, "
                'nullable=False, alias="_id", title=" Id" )'
            }
        if schema_metadata.class_name in add_relation:
            additional_sql_data[schema_metadata.class_name]["SQL"]["relations"] = add_relation[
                schema_metadata.class_name
            ]
        if schema_metadata.class_name in relation_imports:
            additional_sql_data[schema_metadata.class_name]["SQL"]["relationimports"] = relation_imports[
                schema_metadata.class_name
            ]
        if schema_metadata.class_name + "ADD" in relation_imports:
            additional_sql_data[schema_metadata.class_name]["SQL"]["imports"] = relation_imports[
                schema_metadata.class_name + "ADD"
            ]
    additional_arguments["extra_template_data"] = additional_sql_data
    additional_arguments["additional_imports"] = [
        "sqlmodel.Field",
        "uuid as uuid_pkg",
        "sqlmodel.Relationship",
        "sqlmodel._compat.SQLModelConfig",
    ]
    additional_arguments["base_class"] = "sqlmodel.SQLModel"
    additional_arguments["custom_template_dir"] = Path(__file__).resolve().parent / Path("custom_templates")

    return additional_arguments


def return_ref(dictionary: dict[str, Union[str, dict]], target_key: str) -> str:
    """
    returns name of class which is referenced
    """
    for key, value in dictionary.items():
        if key == target_key and isinstance(value, str):
            return (value.split("/")[-1]).split(".json")[0]
        if isinstance(value, dict):
            return return_ref(value, target_key)
        if isinstance(value, list):
            for item in value:
                if isinstance(item, dict):
                    return return_ref(item, target_key)
    return ""


def create_sql_list(
    field_name: str,
    class_name: str,
    namespace: dict[str, SchemaMetadata],
    add_fields: DefaultDict[str, dict[str, Any]],
    add_imports: DefaultDict[str, dict[str, str]],
) -> tuple[DefaultDict[str, dict[str, Any]], DefaultDict[str, dict[str, str]]]:
    """
    Parses and recreates field which contains a list but no reference.
    """
    field_from_json = namespace[class_name].schema_parsed["properties"][field_name]
    default = None
    is_optional = ""
    list_type = None
    typing_dict: dict[str, str] = {"string": "str", "integer": "int"}
    if "default" in field_from_json and field_from_json["default"] != "null" and field_from_json["default"] is not None:
        default = f'{field_from_json["default"]}'
    if "anyOf" in field_from_json:
        for item in field_from_json["anyOf"]:
            if "type" in item:
                if item["type"] == "null":
                    is_optional = "| None"
                if item["type"] == "array":
                    if "type" in item["items"]:
                        list_type = item["items"]["type"]
    # get rid of underscore in fieldname
    field_name = field_name.lstrip("_")
    # transform lists to sqlalchemy arrays
    if list_type not in typing_dict:
        raise KeyError(f"transforming SQL list: {list_type} not known")
    type_hint = typing_dict[list_type]
    sa_type = list_type.capitalize()
    add_imports[class_name + "ADD"]["Column, ARRAY"] = "sqlalchemy"
    add_imports[class_name + "ADD"][sa_type] = "sqlalchemy"

    add_fields[class_name][f"{camel_to_snake(field_name)}"] = (
        f"List[{type_hint}] "
        + is_optional
        + f' = Field({default}, title="{field_name}", sa_column=Column( ARRAY( {sa_type} )))'
    )

    return add_fields, add_imports


# pylint: disable=too-many-arguments
def sql_reference_enum(
    is_list: bool,
    class_name: str,
    field_name: str,
    reference_name: str,
    is_optional: str,
    default: str | None,
    namespace: dict[str, SchemaMetadata],
    add_fields: DefaultDict[str, dict[str, Any]],
    add_imports: DefaultDict[str, dict[str, str]],
) -> tuple[DefaultDict[str, dict[str, Any]], DefaultDict[str, dict[str, str]]]:
    """
    returns field which references enums.
    """
    if is_list:
        add_fields[class_name][f"{camel_to_snake(field_name)}"] = (
            f"List[{reference_name}]" + is_optional + f" = Field({default},"
            f' sa_column=Column( ARRAY( Enum( {reference_name}, name="{reference_name.lower()}"))))'
        )
    else:
        add_fields[class_name][f"{camel_to_snake(field_name)}"] = (
            f"{reference_name}" + is_optional + f"= Field({default})"
        )

    # import enums
    if is_list:
        add_imports[class_name + "ADD"]["Column, ARRAY, Enum"] = "sqlalchemy"
        add_imports[class_name + "ADD"]["List"] = "typing"
    add_imports[class_name + "ADD"][reference_name] = ".".join(namespace[reference_name].module_path)

    return add_fields, add_imports


def create_sql_field(
    field_name: str,
    class_name: str,
    namespace: dict[str, SchemaMetadata],
    add_fields: DefaultDict[str, dict[str, Any]],
    add_imports: DefaultDict[str, dict[str, str]],
) -> tuple[DefaultDict[str, dict[str, Any]], DefaultDict[str, dict[str, str]]]:
    """
    Parses field with references to other classes, enums and lists, and converts it to SQLModel field.
    """

    field_from_json = namespace[class_name].schema_parsed["properties"][field_name]
    reference_name = return_ref(field_from_json, "$ref")
    default = None
    is_optional = ""
    is_list = False
    if "default" in field_from_json and field_from_json["default"] != "null" and field_from_json["default"] is not None:
        default = f'{reference_name}.{field_from_json["default"]}'
    if "anyOf" in field_from_json:
        for item in field_from_json["anyOf"]:
            if "type" in item:
                if item["type"] == "null":
                    is_optional = "| None"
                if item["type"] == "array":
                    is_list = True
    # get rid of underscore in fieldname
    field_name = field_name.lstrip("_")

    if namespace[reference_name].module_path[0] == "enum":
        return sql_reference_enum(
            is_list, class_name, field_name, reference_name, is_optional, default, namespace, add_fields, add_imports
        )
    add_imports[class_name + "ADD"]["List"] = "typing"
    add_imports[reference_name + "ADD"]["List"] = "typing"
    if is_list:
        # create many-to-many class
        if class_name not in add_fields["MANY"]:
            add_fields["MANY"][class_name] = [[reference_name, field_name]]
        elif reference_name not in add_fields["MANY"][class_name]:
            add_fields["MANY"][class_name].append([reference_name, field_name])
        add_fields[class_name][f"{camel_to_snake(field_name)}"] = (
            f'List["{reference_name}"] ='
            f' Relationship(back_populates="{class_name.lower()}_{field_name.lower()}_link", '
            f"link_model={class_name}{field_name}Link)"
        )
        add_fields[reference_name][f"{class_name.lower()}_{field_name.lower()}_link"] = (
            f'List["{class_name}"] ='
            f' Relationship(back_populates="{camel_to_snake(field_name)}", '
            f"link_model={class_name}{field_name}Link)"
        )
        add_imports[class_name + "ADD"][f"{class_name}{field_name}Link)"] = "Link"
        add_imports[reference_name + "ADD"][f"{class_name}{field_name}Link)"] = "Link"
    else:
        # cf. https://github.com/tiangolo/sqlmodel/pull/610
        add_fields[class_name][f"{camel_to_snake(field_name)}_id"] = (
            "uuid_pkg.UUID " + is_optional + f" = Field(sa_column=Column(UUID(as_uuid=True),"
            f' ForeignKey("{reference_name.lower()}.id"'
            f', ondelete="SET NULL")))'
        )
        add_imports[class_name + "ADD"]["Column"] = "sqlalchemy"
        add_imports[class_name + "ADD"]["ForeignKey"] = "sqlalchemy"
        add_imports[class_name + "ADD"]["UUID"] = "sqlalchemy.dialects.postgresql"

        # pylint: disable= fixme
        # todo: check default

        add_fields[class_name][f"{camel_to_snake(field_name)}"] = (
            f'"{reference_name}" ='
            f' Relationship(back_populates="{class_name.lower()}_{camel_to_snake(field_name)}",'
            f' sa_relationship_kwargs= {{ "foreign_keys":"[{class_name}.{camel_to_snake(field_name)}_id]" }})'
        )

        # cf. https://github.com/tiangolo/sqlmodel/issues/10
        # https://github.com/tiangolo/sqlmodel/issues/213
        # https://dev.to/whchi/disable-sqlmodel-foreign-key-constraint-55kp
        add_fields[reference_name][f"{class_name.lower()}_{camel_to_snake(field_name)}"] = (
            f'List["{class_name}"] = Relationship(back_populates="{camel_to_snake(field_name)}",'
            f"sa_relationship_kwargs="
            f'{{"primaryjoin":'
            f' "{class_name}.{camel_to_snake(field_name)}_id=={reference_name}.id",'
            f' "lazy": "joined"}})'
        )
    # add_relation_import
    add_imports[class_name][reference_name] = ".".join(namespace[reference_name].module_path)
    add_imports[reference_name][class_name] = ".".join(namespace[class_name].module_path)

    return add_fields, add_imports


def create_sql_any(
    field_name: str,
    class_name: str,
    namespace: dict[str, SchemaMetadata],
    add_fields: DefaultDict[str, dict[str, Any]],
    add_imports: DefaultDict[str, dict[str, str]],
) -> tuple[DefaultDict[str, dict[str, Any]], DefaultDict[str, dict[str, str]]]:
    """
    Parses field with type any, and converts it to SQLModel field., cf.
    https://github.com/tiangolo/sqlmodel/issues/178
    """
    field_from_json = namespace[class_name].schema_parsed["properties"][field_name]
    default = None
    is_optional = ""
    is_list = False
    if "default" in field_from_json and field_from_json["default"] != "null" and field_from_json["default"] is not None:
        default = f'{field_from_json["default"]}'
    if "anyOf" in field_from_json:
        for item in field_from_json["anyOf"]:
            if "type" in item:
                if item["type"] == "null":
                    is_optional = "| None"
                if item["type"] == "array":
                    is_list = True
    # get rid of underscore in fieldname
    field_name = field_name.lstrip("_")

    add_imports[class_name + "ADD"]["Any"] = "typing"
    add_imports[class_name + "ADD"]["Column, PickleType"] = "sqlmodel"
    if is_list:
        add_imports[class_name + "ADD"]["List"] = "typing"
        add_imports[class_name + "ADD"]["ARRAY"] = "sqlalchemy"
        add_fields[class_name][f"{camel_to_snake(field_name)}"] = (
            "List[Any]" + is_optional + f" = Field({default}," f" sa_column=Column( ARRAY( PickleType)))"
        )
    else:
        add_fields[class_name][f"{camel_to_snake(field_name)}"] = (
            "Any" + is_optional + f" = Field({default}," f" sa_column=Column(  PickleType))"
        )

    return add_fields, add_imports


def write_many_many_links(links: dict[str, str]) -> str:
    """
    use template to write many-to-many link classes to many.py file
    """
    template_path = Path(__file__).resolve().parent / Path("custom_templates")
    environment = Environment(loader=FileSystemLoader(template_path))
    template = environment.get_template("ManyLinks.jinja2")
    python_code = template.render({"class": links})
    # python_code = format_code(python_code)
    return python_code


def format_code(code: str) -> str:
    """
    perform isort and black on code
    """
    code = remove_unused_imports(code)
    code = black.format_str(code, mode=black.Mode())
    return isort.code(code)


def remove_unused_imports(code):
    """
    Removes unused imports from the given code using autoflake.
    """
    # Create a temporary file
    with tempfile.NamedTemporaryFile(suffix=".py", delete=False) as tmp_file:
        tmp_file_name = tmp_file.name
        tmp_file.write(code.encode("utf-8"))

    # Run autoflake to remove unused imports
    subprocess.run(["autoflake", "--remove-all-unused-imports", "--in-place", tmp_file_name], check=True)

    # Read the cleaned code from the temporary file
    with open(tmp_file_name, "r", encoding="utf-8") as tmp_file:
        cleaned_code = tmp_file.read()

    # Clean up the temporary file
    os.remove(tmp_file_name)

    return cleaned_code
