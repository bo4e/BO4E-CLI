"""
This module contains the models for the GitHub API queries.
"""

from pathlib import Path
from types import MappingProxyType
from typing import (
    Annotated,
    Callable,
    Dict,
    Hashable,
    ItemsView,
    Iterable,
    Iterator,
    KeysView,
    Mapping,
    Set,
    TypeVar,
    Union,
    ValuesView,
)

from _weakrefset import WeakSet
from pydantic import BaseModel, Field, HttpUrl, RootModel, computed_field, model_validator

from bo4e_cli.models.schema import SchemaRootType


class SchemaMeta(BaseModel):
    """
    A schema in the file tree returned by the GitHub API. Only contains the relevant information.
    """

    name: str
    """ E.g. 'Marktlokation' """
    module: tuple[str, ...]
    """ E.g. ('bo', 'Marktlokation') or ('ZusatzAttribut',) """
    src: Path | HttpUrl
    """ Either an online URL or a local file path """

    _schema: SchemaRootType | str | None = None

    @computed_field
    @property
    def relative_path(self) -> Path:
        """E.g. 'bo/Marktlokation.json' or 'ZusatzAttribut.json'"""
        return Path(*self.module).with_suffix("json")

    @property
    def src_url(self) -> HttpUrl:
        if not isinstance(self.src, HttpUrl):
            raise ValueError("The source is not an online URL.")
        return self.src

    @property
    def src_path(self) -> Path:
        if not isinstance(self.src, Path):
            raise ValueError("The source is not a local file path.")
        return self.src

    @property
    def schema_parsed(self) -> SchemaRootType:
        if self._schema is None:
            raise ValueError("The schema has not been loaded yet. Set `schema_parsed` or `schema_text` first.")
        if isinstance(self._schema, str):
            self._schema = SchemaRootType.model_validate_json(self._schema_text)
        return self._schema

    @schema_parsed.setter
    def schema_parsed(self, value: SchemaRootType):
        self._schema = value

    @schema_parsed.deleter
    def schema_parsed(self):
        self._schema = None

    @property
    def schema_text(self) -> str:
        if self._schema is None:
            raise ValueError("The schema has not been loaded yet. Set `schema_parsed` or `schema_text` first.")
        if isinstance(self._schema, SchemaRootType):
            return self._schema.model_dump_json(indent=2, exclude_unset=True, by_alias=True)
        return self._schema

    @schema_text.setter
    def schema_text(self, value: str):
        if isinstance(self._schema, SchemaRootType):
            raise ValueError(
                "The schema has already been parsed. If you are sure you want to delete possible changes "
                "to the parsed schema, delete `schema_parsed` first."
            )
        self._schema = value


T = TypeVar("T", bound=Hashable)


class Schemas(RootModel[set[SchemaMeta]], Set[SchemaMeta]):
    root: Annotated[set[SchemaMeta], Field(default_factory=set)]

    _search_indices: WeakSet["SearchIndex[str]"] = WeakSet()

    @property
    def search_index_by_cls_name(self) -> "SearchIndex[str]":
        search_index = SearchIndex(self, key_func=lambda schema: schema.name)
        self._search_indices.add(search_index)
        return search_index

    @property
    def search_index_by_module(self) -> "SearchIndex[tuple[str, ...]]":
        search_index = SearchIndex(self, key_func=lambda schema: schema.module)
        self._search_indices.add(search_index)
        return search_index

    @property
    def search_index_by_src_path(self) -> "SearchIndex[Path]":
        search_index = SearchIndex(self, key_func=lambda schema: schema.src_path)
        self._search_indices.add(search_index)
        return search_index

    def _flag_search_indices(self) -> None:
        for index in self._search_indices:
            index._schemas_updated = True

    # ****************** Functions to mimic a set ******************
    def __contains__(self, item: object) -> bool:
        return self.root.__contains__(item)

    def __iter__(self) -> Iterator[SchemaMeta]:
        return self.root.__iter__()

    def __len__(self) -> int:
        return self.root.__len__()

    def __le__(self, other: AbstractSet[object]) -> bool:
        return self.root.__le__(other)

    def __lt__(self, other: AbstractSet[object]) -> bool:
        return self.root.__lt__(other)

    def __eq__(self, other: object) -> bool:
        return isinstance(other, Schemas) and self.root.__eq__(other.root)

    def __ne__(self, other: object) -> bool:
        return not self.__eq__(other)

    def __gt__(self, other: AbstractSet[object]) -> bool:
        return self.root.__gt__(other)

    def __ge__(self, other: AbstractSet[object]) -> bool:
        return self.root.__ge__(other)

    def __and__(self, other: AbstractSet[object]) -> Set[SchemaMeta]:
        return self.root.__and__(other)

    def __or__(self, other: AbstractSet[T]) -> Set[SchemaMeta | T]:
        return self.root.__or__(other)

    def __sub__(self, other: AbstractSet[SchemaMeta | None]) -> Set[SchemaMeta]:
        return self.root.__sub__(other)

    def __xor__(self, other: AbstractSet[T]) -> Set[SchemaMeta | T]:
        return self.root.__xor__(other)

    def isdisjoint(self, other: Iterable[object]) -> bool:
        return self.root.isdisjoint(other)

    def add(self, item: SchemaMeta) -> None:
        prev_len = len(self.root)  # To prevent double contain check. This should be faster.
        self.root.add(item)
        if len(self.root) != prev_len:
            self._flag_search_indices()

    def update(self, *items_iters: Iterable[SchemaMeta]) -> None:
        prev_len = len(self.root)  # To prevent double contain check. This should be faster.
        self.root.update(*items_iters)
        if len(self.root) != prev_len:
            self._flag_search_indices()

    def remove(self, item: SchemaMeta) -> None:
        prev_len = len(self.root)  # To prevent double contain check. This should be faster.
        self.root.remove(item)
        if len(self.root) != prev_len:
            self._flag_search_indices()


class SearchIndex(Mapping[T, SchemaMeta]):
    def __init__(self, schemas: Schemas, key_func: Callable[[SchemaMeta], T]):
        self._schemas = schemas
        self._schemas_updated = False
        self._key_func = key_func
        self._index: dict[T, SchemaMeta]
        self._build_index()

    def _build_index(self) -> None:
        self._index = {}
        for schema in self._schemas:
            key = self._key_func(schema)
            if key in self._index:
                raise ValueError(f"Duplicate key: {key}")
            self._index[key] = schema

    def _update_index_if_flagged(self) -> None:
        if self._schemas_updated:
            self._build_index()
            self._schemas_updated = False

    # ****************** Functions to mimic a mapping ******************
    def __getitem__(self, item: T) -> SchemaMeta:
        self._update_index_if_flagged()
        return self._index.__getitem__(item)

    def __iter__(self) -> Iterator[T]:
        self._update_index_if_flagged()
        return self._index.__iter__()

    def __len__(self) -> int:
        return len(self._schemas)

    def __contains__(self, other: object) -> bool:
        self._update_index_if_flagged()
        return self._index.__contains__(other)

    def keys(self) -> KeysView[T]:
        self._update_index_if_flagged()
        return self._index.keys()

    def items(self) -> ItemsView[T, SchemaMeta]:
        self._update_index_if_flagged()
        return self._index.items()

    def values(self) -> ValuesView[SchemaMeta]:
        return self._schemas

    def get(self, key: T, default: SchemaMeta | None = None) -> SchemaMeta | None:
        self._update_index_if_flagged()
        return self._index.get(key, default)

    def __eq__(self, other: object) -> bool:
        if not isinstance(other, SearchIndex):
            return False
        self._update_index_if_flagged()
        other._update_index_if_flagged()
        return self._index.__eq__(other._index)

    def __ne__(self, other: object) -> bool:
        return not self.__eq__(other)


class SchemaTree(RootModel, Dict[str, Union["SchemaTree", SchemaMeta]]):
    """
    This model represents a file tree of `SchemaInFileTree` objects.
    You can use path indices to access the tree. The class will handle those paths and splits them
    into separate indices.
    """

    root: Annotated[dict[str, Union["SchemaTree", SchemaMeta]], Field(default_factory=dict)]
    _namespace_index: Annotated[dict[str, SchemaMeta], Field(default_factory=dict)]

    @model_validator(mode="after")
    def init_namespace_index(self):
        self._namespace_index = {}
        for schema_meta in self.all_files():
            if schema_meta.name in self._namespace_index:
                raise ValueError(f"Duplicate schema name: {schema_meta.name}")
            self._namespace_index[schema_meta.name] = schema_meta

    @staticmethod
    def resolve_path(path: str) -> list[str]:
        """
        Splits a path into its parts.
        """
        return path.split("/")

    def __setitem__(self, key, value):
        if not isinstance(value, SchemaMeta):
            raise ValueError("Only SchemaInFileTree objects are allowed in the tree.")
        if value.name in self._namespace_index:
            raise ValueError(f"Duplicate schema name: {value.name}")
        parts = self.resolve_path(key)
        current = self.root
        for part in parts[:-1]:
            try:
                current = current[part]
            except KeyError:
                current[part] = self.__class__()
                current = current[part]
        current[parts[-1]] = value
        self._namespace_index[value.name] = value

    def __getitem__(self, key):
        parts = self.resolve_path(key)
        current = self.root
        for part in parts:
            try:
                current = current[part]
            except KeyError:
                current[part] = self.__class__()
                current = current[part]
        return current

    def __contains__(self, path):
        parts = self.resolve_path(path)
        current = self.root
        for part in parts:
            if part not in current:
                return False
            current = current[part]
        return True

    def __iter__(self):
        return iter(self.root)

    def __len__(self):
        return len(self.root)

    def keys(self) -> KeysView[str]:
        """Get all keys of the root."""
        return self.root.keys()

    def values(self) -> ValuesView[Union["SchemaTree", SchemaMeta]]:
        """Get all values of the root."""
        return self.root.values()

    def items(self) -> ItemsView[str, Union["SchemaTree", SchemaMeta]]:
        """Get all items of the root."""
        return self.root.items()

    def all_files(self) -> Iterable[SchemaMeta]:
        """Get all files in the schema tree."""
        for value in self.values():
            if isinstance(value, SchemaMeta):
                yield value
            else:
                yield from value.all_files()

    @property
    def namespace(self) -> MappingProxyType[str, SchemaMeta]:
        return MappingProxyType(self._namespace_index)
