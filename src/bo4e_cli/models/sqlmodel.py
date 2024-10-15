"""
This module contains the models used for the SQLModel generation
"""

import inspect
from collections import defaultdict
from importlib import import_module
from pathlib import Path
from typing import Any, Collection, Iterable, Iterator, TypeAlias

from datamodel_code_generator.imports import Import
from pydantic import BaseModel, Field, GetCoreSchemaHandler
from pydantic_core import CoreSchema, core_schema

from bo4e_cli.utils.data_structures import RootModelDict


class Imports(Collection[Import]):
    """A set-like collection of imports

    Can be (de-)serialized with pydantic. Ensures that the set of imports doesn't result in naming conflicts.
    """

    def __init__(self) -> None:
        self._names: dict[str, Import] = {}

    def __contains__(self, item: object) -> bool:
        return item in self._names.values()

    def __len__(self) -> int:
        return len(self._names)

    def __iter__(self) -> Iterator[Import]:
        return iter(self._names.values())

    @staticmethod
    def _import_local_name(import_: Import) -> str:
        """Determines the local name for an import

        Args:
            import_: The import to determine the local name for
        """
        return import_.alias or import_.import_

    def _has_name(self, import_: Import) -> bool:
        """Checks if an import with the same local name is already present in this collection

        Args:
            import_: The import to check
        """
        return self._import_local_name(import_) in self._names

    def add(self, import_: Import) -> None:
        """Adds an import to the collection

        Args:
            import_: The import to add
        Raises:
            ValueError: If an import with the same local name is already present in this collection _except_ if it's the
                same import. In that case, the function is a no-op.
        """
        import_name = self._import_local_name(import_)
        if self._has_name(import_):
            if self._names[import_name] != import_:
                raise ValueError(f"Duplicate import name: {import_name}")
            return  # ignore duplicate imports
        self._names[import_name] = import_

    def update(self, imports: Iterable[Import]) -> None:
        """Adds multiple imports to the collection

        Args:
            imports: The imports to add
        Raises:
            ValueError: If an import with the same local name is already present in this collection _except_ if it's the
                same import. In that case, the import will be skipped.
        """
        for import_ in imports:
            self.add(import_)

    # pylint: disable=unused-argument
    @classmethod
    def __get_pydantic_core_schema__(cls, source_type: Any, handler: GetCoreSchemaHandler) -> CoreSchema:
        """Get the core schema for this collection

        This method is used by pydantic to serialize and validate the collection

        Args:
            handler: The handler to use to get the core schema
        """
        list_schema = core_schema.list_schema(Import.__pydantic_core_schema__)
        return core_schema.json_or_python_schema(
            json_schema=list_schema,
            python_schema=core_schema.union_schema(
                [
                    # check if it's an instance first before doing any further work
                    core_schema.is_instance_schema(Imports),
                    list_schema,
                ]
            ),
        )


class SQLModelField(BaseModel):
    """Class representing a field in a SQLModel

    Attributes:
        name: The name of the field
        annotation: The type hint of the field
        definition: The definition of the field
        description: The description / docstring of the field
    """

    name: str
    annotation: str
    definition: str
    description: str | None


class SQLModelTemplateDataPerModel(BaseModel):
    """Class representing the template data for a single model

    Attributes:
        fields: The fields of the model which were handled by the sql_parser
        imports: The additional imports needed for the model
        imports_forward_refs: The imports needed for forward references. Should be branched out using the TYPE_CHECKING
            flag.
    """

    fields: dict[str, SQLModelField] = Field(default_factory=dict)
    imports: Imports = Field(default_factory=Imports)
    imports_forward_refs: Imports = Field(default_factory=Imports)


class ExtraTemplateDataPerModel(BaseModel):
    """Class representing the extra template data for a single model

    Attributes:
        sql: The SQLModel template data for the model. They are enclosed inside this field to avoid name conflicts
            with other template data from datamodel-code-generator.
    """

    sql: SQLModelTemplateDataPerModel = Field(default_factory=SQLModelTemplateDataPerModel, alias="SQL")


class ExtraTemplateData(RootModelDict[str, ExtraTemplateDataPerModel]):
    """Class representing the extra template data for all models

    Attributes:
        root: The extra template data for all models as a dictionary (model name -> extra template data)
    """

    root: dict[str, ExtraTemplateDataPerModel] = Field(default_factory=lambda: defaultdict(ExtraTemplateDataPerModel))


class AdditionalParserKwargs(BaseModel):
    """This class is used to pass additional keyword arguments to the parser of datamodel-code-generator

    Attributes:
        base_class: The base class to use for the generated models
        custom_template_dir: The directory containing the custom jinja templates for datamodel-code-generator
        additional_imports: Additional imports needed for all generated models
        extra_template_data: Extra template data for each model
    """

    base_class: str = "sqlmodel.SQLModel"
    custom_template_dir: Path = (
        Path(inspect.getfile(import_module("bo4e_cli.generate.python"))).parent / "custom_templates"
    )
    additional_imports: list[str] = Field(
        default_factory=lambda: [
            "sqlmodel.Field",
            "uuid as uuid_pkg",
            "sqlmodel._compat.SQLModelConfig",
        ]
    )
    extra_template_data: ExtraTemplateData = Field(default_factory=ExtraTemplateData)


class ManyToManyRelationship(BaseModel):
    """Class representing a many-to-many relationship

    Attributes:
        table_name: The name of the link table
        cls1: The name of the first class
        cls2: The name of the second class
        rel_field_name1: The name of the relationship field in the first class
        rel_field_name2: The name of the relationship field in the second class (if you want to have a bidirectional
            relationship)
        id_field_name1: The name of the id field in the link table that references the first class
        id_field_name2: The name of the id field in the link table that references the second class
    """

    table_name: str
    cls1: str
    cls2: str
    rel_field_name1: str | None
    rel_field_name2: str | None
    id_field_name1: str
    id_field_name2: str


ManyToManyRelationships: TypeAlias = list[ManyToManyRelationship]
"""Type for a list of many-to-many relationships"""
