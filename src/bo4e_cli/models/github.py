"""
This module contains the models for the GitHub API queries.
"""

from types import MappingProxyType
from typing import Annotated, Dict, ItemsView, Iterable, KeysView, Mapping, Union, ValuesView

from pydantic import BaseModel, Field, RootModel, model_validator


class SchemaInFileTree(BaseModel):
    """
    A schema in the file tree returned by the GitHub API. Only contains the relevant information.
    """

    name: str
    path: str
    module_path: tuple[str, ...]
    download_url: str


class SchemaTree(RootModel, Dict[str, Union["SchemaTree", SchemaInFileTree]]):
    """
    This model represents a file tree of `SchemaInFileTree` objects.
    You can use path indices to access the tree. The class will handle those paths and splits them
    into separate indices.
    """

    root: Annotated[dict[str, Union["SchemaTree", SchemaInFileTree]], Field(default_factory=dict)]
    _namespace_index: Annotated[dict[str, SchemaInFileTree], Field(default_factory=dict)]

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
        if not isinstance(value, SchemaInFileTree):
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

    def values(self) -> ValuesView[Union["SchemaTree", SchemaInFileTree]]:
        """Get all values of the root."""
        return self.root.values()

    def items(self) -> ItemsView[str, Union["SchemaTree", SchemaInFileTree]]:
        """Get all items of the root."""
        return self.root.items()

    def all_files(self) -> Iterable[SchemaInFileTree]:
        """Get all files in the schema tree."""
        for value in self.values():
            if isinstance(value, SchemaInFileTree):
                yield value
            else:
                yield from value.all_files()

    @property
    def namespace(self) -> MappingProxyType[str, SchemaInFileTree]:
        return MappingProxyType(self._namespace_index)
