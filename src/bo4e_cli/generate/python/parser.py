"""
Contains code to generate pydantic v2 models from json schemas.
Since the used tool doesn't support all features we need, we monkey patch some functions.
"""

import itertools
import re
import shutil
from enum import Enum
from pathlib import Path
from typing import Any, Sequence, Type

import datamodel_code_generator.parser.base
import datamodel_code_generator.reference
from datamodel_code_generator import DataModelType, PythonVersion
from datamodel_code_generator.format import CodeFormatter
from datamodel_code_generator.imports import IMPORT_DATETIME
from datamodel_code_generator.model import DataModel, DataModelSet, get_data_model_types
from datamodel_code_generator.model.enum import Enum as _Enum
from datamodel_code_generator.parser.jsonschema import JsonSchemaParser
from datamodel_code_generator.types import DataType, StrictTypes, Types

from bo4e_cli.generate.python.imports import monkey_patch_imports
from bo4e_cli.generate.python.schema import SchemaMetadata
from bo4e_cli.generate.python.sqlparser import adapt_parse_for_sql, remove_pydantic_field_import, write_many_many_links


class OutputType(str, Enum):
    """
    enum to specify the output type
    """

    PYDANTIC_V2 = "pydantic_v2"
    PYDANTIC_V1 = "pydantic_v1"
    SQL_MODEL = "sql_model"


def get_bo4e_data_model_types(
    data_model_type: DataModelType,
    target_python_version: PythonVersion,
    namespace: dict[str, SchemaMetadata],
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
        if self.name not in namespace:
            raise ValueError(f"Model not in namespace: {self.name}")
        return list(namespace[self.name].module_path)

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
                    if self.reference and type_ in namespace and namespace[type_].module_path[0] != "enum":
                        type_ = f'"{type_}"'
                    return type_

            self.data_type = DataTypeWithForwardRef

        def type_map_factory(
            self,
            data_type: Type[DataType],
            strict_types: Sequence[StrictTypes],
            pattern_key: str,
        ) -> dict[Types, DataType]:
            """overwrite the AwareDatetime import"""
            result: dict[Types, DataType] = super().type_map_factory(data_type, strict_types, pattern_key)
            result[Types.date_time] = data_type.from_import(IMPORT_DATETIME)
            return result

    monkey_patch_imports(namespace)

    return DataModelSet(
        data_model=BO4EDataModel,
        root_model=data_model_types.root_model,
        field_model=data_model_types.field_model,
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
BO4E v{version} - Generated Python implementation of the BO4E standard

BO4E is a standard for the exchange of business objects in the energy industry.
All our software used to generate this BO4E-implementation is open-source and released under the Apache-2.0 license.

The BO4E version can be queried using `bo4e.__version__`.
"""
'''


def bo4e_init_file_content(namespace: dict[str, SchemaMetadata], version: str) -> str:
    """
    Create __init__.py files in all subdirectories of the given output directory and in the directory itself.
    """
    init_file_content = INIT_FILE_COMMENT.strip().format(version=version)

    init_file_content += "\n\n__all__ = [\n"
    for class_name in sorted(itertools.chain(namespace, ["__version__"])):
        init_file_content += f'    "{class_name}",\n'
    init_file_content += "]\n\n"

    for schema_metadata in namespace.values():
        init_file_content += f"from .{'.'.join(schema_metadata.module_path)} import {schema_metadata.class_name}\n"
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
    return re.sub(rf"{class_name}\.model_rebuild\(\)\n", "", python_code)


def parse_bo4e_schemas(
    input_directory: Path, namespace: dict[str, SchemaMetadata], output_type: OutputType
) -> dict[Path, str]:
    """
    Generate all BO4E schemas from the given input directory. Returns all file contents as dictionary:
    file path (relative to arbitrary output directory) => file content.
    """
    data_model_types = get_bo4e_data_model_types(
        (
            DataModelType.PydanticBaseModel
            if output_type is OutputType.PYDANTIC_V1.name
            else DataModelType.PydanticV2BaseModel
        ),
        target_python_version=PythonVersion.PY_311,
        namespace=namespace,
    )
    monkey_patch_relative_import()

    additional_arguments: dict[str, Any] = {}

    if output_type is OutputType.SQL_MODEL.name:
        # adapt input for SQLModel classes
        namespace, additional_arguments, input_directory, links = adapt_parse_for_sql(input_directory, namespace)

    parser = JsonSchemaParser(
        input_directory,
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
        base_path=input_directory,
        remove_special_field_name_prefix=True,
        allow_extra_fields=False,
        allow_population_by_field_name=True,
        use_default_kwarg=True,
        strict_nullable=True,
        **additional_arguments,
    )
    parse_result = parser.parse()
    if not isinstance(parse_result, dict):
        raise ValueError(f"Unexpected type of parse result: {type(parse_result)}")
    file_contents = {}
    for schema_metadata in namespace.values():
        module_path = schema_metadata.module_path_with_extension
        if schema_metadata.module_name.startswith("_"):
            # Because somehow the generator uses the prefix also on the module name. Don't know why.
            module_path = *module_path[:-1], f"field{module_path[-1]}"

        if module_path not in parse_result:
            raise KeyError(
                f"Could not find module {'.'.join(module_path)} in results: "
                f"{list(parse_result.keys())}"
                # Item "str" of "str | dict[tuple[str, ...], Result]" has no attribute "keys"
                # Somehow, mypy is not good enough to understand the instance-check above
            )

        python_code = remove_future_import(parse_result.pop(module_path).body)
        python_code = remove_model_rebuild(python_code, schema_metadata.class_name)
        if output_type is OutputType.SQL_MODEL.name:
            # remove pydantic field
            python_code = remove_pydantic_field_import(python_code)

        file_contents[schema_metadata.output_file] = python_code

    file_contents.update({Path(*module_path): result.body for module_path, result in parse_result.items()})

    # add SQLModel classes for many-to-many relationships in "many.py"
    if output_type is OutputType.SQL_MODEL.name:
        shutil.rmtree(input_directory)  # remove intermediate dir of schemas
        if links:
            file_contents[Path("many.py")] = write_many_many_links(links)

    return file_contents


def get_formatter() -> CodeFormatter:
    """
    Returns a formatter to apply black and isort
    """
    return CodeFormatter(
        PythonVersion.PY_311,
        None,
        None,
        skip_string_normalization=False,
        known_third_party=None,
        custom_formatters=None,
        custom_formatters_kwargs=None,
    )
