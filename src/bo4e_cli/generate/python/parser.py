"""
Contains code to generate pydantic v2 models from json schemas.
Since the used tool doesn't support all features we need, we monkey patch some functions.
"""

import itertools
import re
import shutil
import tempfile
from pathlib import Path
from typing import Any, Sequence, Type

import datamodel_code_generator.parser.base
import datamodel_code_generator.reference
from datamodel_code_generator import DataModelType, LiteralType, PythonVersion
from datamodel_code_generator.format import DatetimeClassType
from datamodel_code_generator.imports import IMPORT_DATETIME, Import
from datamodel_code_generator.model import DataModel, DataModelFieldBase, DataModelSet, get_data_model_types
from datamodel_code_generator.model.enum import Enum as _Enum
from datamodel_code_generator.parser.jsonschema import JsonSchemaParser
from datamodel_code_generator.types import DataType, StrictTypes, Types

from bo4e_cli.generate.python.imports import monkey_patch_imports
from bo4e_cli.generate.python.sql_parser import adapt_parse_for_sql_model, parse_many_many_links
from bo4e_cli.io.cleanse import clear_dir_if_needed
from bo4e_cli.io.console import CONSOLE
from bo4e_cli.io.schemas import write_schemas
from bo4e_cli.models.meta import Schemas
from bo4e_cli.models.sqlmodel import AdditionalParserKwargs, ManyToManyRelationships
from bo4e_cli.types import GenerateType


def get_bo4e_data_model_types(
    data_model_type: DataModelType,
    target_python_version: PythonVersion,
    schemas: Schemas,
    generate_type: GenerateType,
    monkey_patch_enum_type: bool = True,
) -> DataModelSet:
    """
    Get the data model types for the data model parser.
    In the first place, it overrides functions such that the namespace of the bo4e-package will be recreated instead of
    "put everything in one file".
    """
    data_model_types = get_data_model_types(data_model_type, target_python_version=target_python_version)

    @property  # type: ignore[misc]
    # "property" used with a non-method
    def _module_path(self: DataModel) -> list[str]:
        if self.name not in schemas.names:
            raise ValueError(f"Model not in namespace: {self.name}")
        return list(schemas.names[self.name].python_module)

    @property  # type: ignore[misc]
    # "property" used with a non-method
    def _module_name(self: DataModel) -> str:
        return ".".join(self.module_path)

    # pylint: disable=too-few-public-methods
    class BO4EDataModel(data_model_types.data_model):  # type: ignore[name-defined,misc]
        # Name "data_model_types.data_model" is not defined
        """Override the data model to use create the namespace."""

        module_path = _module_path
        module_name = _module_name

    if monkey_patch_enum_type:
        setattr(_Enum, "module_path", _module_path)
        setattr(_Enum, "module_name", _module_name)

    # pylint: disable=too-few-public-methods
    class BO4EDataTypeManager(data_model_types.data_type_manager):  # type: ignore[name-defined,misc]
        """
        Override the data type manager to prevent the code generator from using the `AwareDateTime` type
        featured in pydantic v2. Instead, the standard datetime type will be used.
        """

        def __init__(self, *args: Any, **kwargs: Any) -> None:
            super().__init__(*args, **kwargs)

            # pylint: disable=too-few-public-methods
            class DataTypeWithForwardRef(self.data_type):  # type: ignore[name-defined,misc]
                """
                Override the data type to replace explicit type references with forward references if the type
                is present in namespace.
                Also, the AwareDateTime import is replaced with the standard datetime import.
                """

                @property
                def type_hint(self) -> str:
                    """Return the type hint for the data type."""
                    type_: str = super().type_hint
                    if self.reference and type_ in schemas.names and schemas.names[type_].module[0] != "enum":
                        type_ = f'"{type_}"'
                    return type_

            self.data_type = DataTypeWithForwardRef

        def type_map_factory(
            self,
            data_type: Type[DataType],
            strict_types: Sequence[StrictTypes],
            pattern_key: str,
            target_datetime_class: DatetimeClassType,  # noqa: ARG002
        ) -> dict[Types, DataType]:
            """overwrite the AwareDatetime import"""
            result: dict[Types, DataType] = super().type_map_factory(
                data_type, strict_types, pattern_key, target_datetime_class
            )
            result[Types.date_time] = data_type.from_import(IMPORT_DATETIME)
            return result

    # pylint: disable=too-few-public-methods
    class SQLModelDataModelField(data_model_types.field_model):  # type: ignore[name-defined,misc]
        """
        Override the data model field to not use the Field class from pydantic. This class will be used only for
        sqlmodel output. In this case, the Field class from sqlmodel will be used.
        """

        @property
        def imports(self) -> tuple[Import, ...]:
            """Return the imports needed for the data model field."""
            return DataModelFieldBase.imports.fget(self)  # type: ignore[no-any-return,attr-defined]

    monkey_patch_imports(schemas)

    return DataModelSet(
        data_model=BO4EDataModel,
        root_model=data_model_types.root_model,
        field_model=(
            data_model_types.field_model if generate_type != GenerateType.PYTHON_SQL_MODEL else SQLModelDataModelField
        ),
        data_type_manager=BO4EDataTypeManager,
        dump_resolve_reference_action=data_model_types.dump_resolve_reference_action,
        known_third_party=data_model_types.known_third_party,
    )


def monkey_patch_relative_import() -> None:
    """
    Function taken from datamodel_code_generator.parser.base.
    This function is used to create the relative imports if a referenced model is used in the file.
    Originally, this function would create something like "from ..enum import typ" and a field definition like
    "typ: Annotated[typ.Typ | None, Field(alias='_typ')] = None".
    This is in general a valid way to do it, but pydantic somehow doesn't like it. It will throw an error if you
    attempt to import an enum this way. Looks something like "'Typ' has no attribute 'Typ'".
    Anyway, this monkey patch changes the imports to "from ..enum.typ import Typ" which resolves the issue.
    """

    def relative(current_module: str, reference: str) -> tuple[str, str]:
        """Find relative module path."""

        current_module_path = current_module.split(".") if current_module else []
        *reference_path, name = reference.split(".")

        if current_module_path == reference_path:
            return "", ""

        i = 0
        for x, y in zip(current_module_path, reference_path):
            if x != y:
                break
            i += 1

        left = "." * (len(current_module_path) - i)
        right = ".".join([*reference_path[i:], name])

        if not left:
            left = "."
        if not right:
            right = name
        elif "." in right:
            extra, right = right.rsplit(".", 1)
            left += extra

        return left, right

    datamodel_code_generator.parser.base.relative = relative


def bo4e_version_file_content(version: str) -> str:
    """
    Create __init__.py files in all subdirectories of the given output directory and in the directory itself.
    """
    return f'""" Contains information about the bo4e version """\n\n__version__ = "{version}"\n'


INIT_FILE_COMMENT = '''
"""
BO4E {version} - Generated Python implementation of the BO4E standard

BO4E is a standard for the exchange of business objects in the energy industry.
All our software used to generate this BO4E-implementation is open-source and released under the Apache-2.0 license.

The BO4E version can be queried using `bo4e.__version__`.
"""
'''


def bo4e_init_file_content(schemas: Schemas) -> str:
    """
    Create __init__.py files in all subdirectories of the given output directory and in the directory itself.
    """
    init_file_content = INIT_FILE_COMMENT.strip().format(version=str(schemas.version))

    init_file_content += "\n\n__all__ = [\n"
    for class_name in sorted(itertools.chain(schemas.names, ["__version__"])):
        init_file_content += f'    "{class_name}",\n'
    init_file_content += "]\n\n"

    for schema in schemas:
        init_file_content += f"from .{'.'.join(schema.python_module)} import {schema.name}\n"
    init_file_content += "\nfrom .__version__ import __version__\n"

    init_file_content += (
        "from pydantic import BaseModel as _PydanticBaseModel\n"
        "\n\n# Resolve all ForwardReferences. This design prevents circular import errors.\n"
        "for cls_name in __all__:\n"
        "    cls = globals().get(cls_name, None)\n"
        "    if cls is None or not isinstance(cls, type) or not issubclass(cls, _PydanticBaseModel):\n"
        "        continue\n"
        "    cls.model_rebuild(force=True)\n"
    )

    return init_file_content


def remove_future_import(python_code: str) -> str:
    """
    Remove the future import from the generated code.
    """
    return re.sub(r"from __future__ import annotations\n\n", "", python_code)


def remove_model_rebuild(python_code: str, class_name: str) -> str:
    """
    Remove the model_rebuild call from the generated code.
    """
    return re.sub(rf"{class_name}\.(?:model_rebuild|update_forward_refs)\(\)\n", "", python_code)
    # This line will be created for pydantic v2/v1 output if the model contains forward refs.
    # In pydantic v2 it's the function model_rebuild, in pydantic v1 it's update_forward_refs.


def parse_bo4e_schemas(schemas: Schemas, generate_type: GenerateType) -> dict[Path, str]:
    """
    Generate all BO4E schemas from the given input directory. Returns all file contents as dictionary:
    file path (relative to arbitrary output directory) => file content.
    """
    tmp_dir_final = tmp_bo4e_dir = Path(tempfile.gettempdir()).resolve() / "bo4e" / "schemas_for_generate_python"
    with CONSOLE.capture():
        clear_dir_if_needed(tmp_dir_final)
    write_schemas(schemas, tmp_bo4e_dir, include_version_file=False, enable_tracker=False)

    data_model_types = get_bo4e_data_model_types(
        data_model_type=(
            DataModelType.PydanticBaseModel
            if generate_type == GenerateType.PYTHON_PYDANTIC_V1
            else DataModelType.PydanticV2BaseModel
        ),
        target_python_version=PythonVersion.PY_312,
        schemas=schemas,
        generate_type=generate_type,
    )
    monkey_patch_relative_import()

    additional_arguments: dict[str, Any] = {}
    additional_parser_kwargs: AdditionalParserKwargs | None = None
    links: ManyToManyRelationships = []

    with CONSOLE.status("Parsing schemas into Python classes", spinner="squish"):
        if generate_type == GenerateType.PYTHON_SQL_MODEL:
            # adapt input for SQLModel classes
            additional_parser_kwargs, tmp_bo4e_dir, links = adapt_parse_for_sql_model(tmp_bo4e_dir, schemas)
            additional_arguments = additional_parser_kwargs.model_dump(mode="python", by_alias=True)
            additional_arguments["extra_template_data"]["#all#"] = {}

        parser = JsonSchemaParser(
            tmp_bo4e_dir,
            data_model_type=data_model_types.data_model,
            data_model_root_type=data_model_types.root_model,
            data_model_field_type=data_model_types.field_model,
            data_type_manager_type=data_model_types.data_type_manager,
            dump_resolve_reference_action=data_model_types.dump_resolve_reference_action,
            # use_annotated=OutputType is not OutputType.PYDANTIC_V1.name,
            use_double_quotes=True,
            use_schema_description=True,
            use_subclass_enum=True,
            use_standard_collections=True,
            use_union_operator=False,
            use_field_description=True,
            set_default_enum_member=True,
            snake_case_field=True,
            field_constraints=True,
            capitalise_enum_members=True,
            base_path=tmp_bo4e_dir,
            remove_special_field_name_prefix=True,
            allow_extra_fields=False,
            allow_population_by_field_name=True,
            use_default_kwarg=True,
            strict_nullable=True,
            enum_field_as_literal=LiteralType.One,
            **additional_arguments,
        )
        parse_result = parser.parse()
    CONSOLE.print("Parsed schemas into Python classes")

    with CONSOLE.status("Validating generated Python modules", spinner="squish"):
        if not isinstance(parse_result, dict):
            raise ValueError(f"Unexpected type of parse result: {type(parse_result)}")
        file_contents = {}
        for schema in schemas:
            module_path = schema.python_module_with_suffix

            if module_path not in parse_result:
                raise KeyError(f"Could not find module {'.'.join(module_path)} in results: {list(parse_result.keys())}")

            python_code = remove_future_import(parse_result.pop(module_path).body)
            python_code = remove_model_rebuild(python_code, schema.name)

            file_contents[schema.python_relative_path] = python_code

        file_contents.update({Path(*module_path): result.body for module_path, result in parse_result.items()})
    CONSOLE.print("Validated generated Python modules")

    # add SQLModel classes for many-to-many relationships in "many.py"
    if generate_type == GenerateType.PYTHON_SQL_MODEL:
        shutil.rmtree(tmp_bo4e_dir)  # remove intermediate dir of schemas
        if len(links) > 0:
            assert additional_parser_kwargs is not None, "Internal error: additional_parser_kwargs is None"
            with CONSOLE.status("Parsing many-to-many relationships into Python classes", spinner="squish"):
                file_contents[Path("many.py")] = parse_many_many_links(
                    links, additional_parser_kwargs.custom_template_dir
                )
            CONSOLE.print("Parsed many-to-many relationships into Python classes")

    shutil.rmtree(tmp_dir_final)  # remove temporary dir of schemas

    return file_contents
