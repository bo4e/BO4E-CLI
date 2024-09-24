import inspect
from importlib import import_module
from pathlib import Path
from typing import Annotated, Any, Collection, DefaultDict, Iterator

from black.trans import defaultdict
from datamodel_code_generator.imports import Import
from datamodel_code_generator.model.pydantic_v2 import RootModel
from pydantic import BaseModel, Field, GetCoreSchemaHandler, PlainSerializer, computed_field, model_serializer
from pydantic_core import CoreSchema, core_schema

from bo4e_cli.utils.root_model import RootModelDict


class Imports(Collection[Import]):
    def __init__(self):
        self._names: dict[str, Import] = {}

    def __contains__(self, item: object) -> bool:
        return item in self._names.values()

    def __len__(self) -> int:
        return len(self._names)

    def __iter__(self) -> Iterator[Import]:
        return iter(self._names.values())

    @staticmethod
    def _import_local_name(import_: Import) -> str:
        return import_.alias or import_.import_

    def _has_name(self, import_: Import) -> bool:
        return self._import_local_name(import_) in self._names

    def add(self, import_: Import) -> None:
        import_name = self._import_local_name(import_)
        if self._has_name(import_):
            if self._names[import_name] != import_:
                raise ValueError(f"Duplicate import name: {import_name}")
            return  # ignore duplicate imports
        self._names[import_name] = import_

    def update(self, imports: Collection[Import]) -> None:
        for import_ in imports:
            self.add(import_)

    def __get_pydantic_core_schema__(self, handler: GetCoreSchemaHandler) -> CoreSchema:
        return handler(list)


def serialize_imports(imports: Imports) -> list[dict[str, str | None]]:
    return [import_.model_dump() for import_ in imports]


ImportsPydanticField = Annotated[Imports, PlainSerializer(serialize_imports)]


class SQLModelField(BaseModel):
    name: str
    annotation: str
    definition: str
    description: str | None


class SQLModelTemplateDataPerModel(BaseModel):
    fields: dict[str, SQLModelField] = Field(default_factory=dict)
    imports: ImportsPydanticField = Field(default_factory=Imports)
    imports_forward_refs: ImportsPydanticField = Field(default_factory=Imports)


class ExtraTemplateDataPerModel(BaseModel):
    sql: SQLModelTemplateDataPerModel = Field(default_factory=SQLModelTemplateDataPerModel, alias="SQL")


class ExtraTemplateData(RootModelDict[str, ExtraTemplateDataPerModel]):
    root: dict[str, ExtraTemplateDataPerModel] = Field(default_factory=lambda: defaultdict(ExtraTemplateDataPerModel))


class AdditionalParserKwargs(BaseModel):
    """
    This class is used to pass additional keyword arguments to the parser
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
    table_name: str
    """The name of the link table"""
    cls1: str
    """The name of the first class"""
    cls2: str
    """The name of the second class"""
    rel_field_name1: str | None
    """The name of the relationship field in the first class"""
    rel_field_name2: str | None
    """The name of the relationship field in the second class (if you want to have a bidirectional relationship)"""
    id_field_name1: str
    """The name of the id field in the link table that references the first class"""
    id_field_name2: str
    """The name of the id field in the link table that references the second class"""


ManyToManyRelationships = list[ManyToManyRelationship]
