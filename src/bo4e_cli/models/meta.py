"""
This module contains the models for the GitHub API queries.
"""

import functools
from collections.abc import Hashable
from pathlib import Path
from typing import Callable, Generic, ItemsView, Iterable, Iterator, KeysView, Literal, TypeVar, ValuesView, overload

from pydantic import BaseModel, ConfigDict, Field, HttpUrl, TypeAdapter

from bo4e_cli.models.schema import SchemaRootObject, SchemaRootStrEnum, SchemaRootType
from bo4e_cli.models.version import Version
from bo4e_cli.models.weakref import WeakCollection
from bo4e_cli.utils.strings import camel_to_snake


class SchemaMeta(BaseModel):
    """
    This class represents a schema meta data object.
    """

    model_config = ConfigDict(frozen=True)

    name: str
    """ E.g. 'Marktlokation' """
    module: tuple[str, ...]
    """ E.g. ('bo', 'Marktlokation') or ('ZusatzAttribut',) """
    src: HttpUrl | Path | None = None
    """ Either an online URL or a local file path. Can be None if this schema is with no source related. """

    _schema: SchemaRootType | str | None = None

    @property
    def python_module(self) -> tuple[str, ...]:
        """e.g. ('bo', 'preisblatt_netznutzung') or ('zusatz_attribut')"""
        return *self.module[:-1], camel_to_snake(self.module[-1])

    @property
    def python_module_with_suffix(self) -> tuple[str, ...]:
        """e.g. ('bo', 'preisblatt_netznutzung.py') or ('zusatz_attribut.py')"""
        return *self.module[:-1], f"{camel_to_snake(self.module[-1])}.py"

    @property
    def python_module_path(self) -> str:
        """e.g. 'bo.preisblatt_netznutzung' or 'zusatz_attribut'"""
        return ".".join(self.python_module)

    @property
    def python_class_path(self) -> str:
        """e.g. 'bo.preisblatt_netznutzung.PreisblattNetznutzung' or 'zusatz_attribut.ZusatzAttribut'"""
        return ".".join(self.python_module) + "." + self.name

    @property
    def relative_path(self) -> Path:
        """E.g. 'bo/Marktlokation.json' or 'ZusatzAttribut.json'"""
        return Path(*self.module).with_suffix(".json")

    @property
    def python_relative_path(self) -> Path:
        """E.g. 'bo/preisblatt_netznutzung.py' or 'zusatz_attribut.py'"""
        return Path(*self.python_module_with_suffix)

    @property
    def src_url(self) -> HttpUrl:
        """Returns the source as an online URL. Raises a ValueError if the source is not a URL."""
        if isinstance(self.src, Path) or self.src is None:
            raise ValueError("The source is not an online URL.")
        return self.src

    @property
    def src_path(self) -> Path:
        """Returns the source as a local file path. Raises a ValueError if the source is not a path."""
        if not isinstance(self.src, Path):
            raise ValueError("The source is not a local file path.")
        return self.src

    @property
    def schema_parsed(self) -> SchemaRootType:
        """
        Returns the parsed schema.
        Raises a ValueError if the schema has not been loaded yet.
        Automatically parses the schema if `set_schema_text` has been called before.
        """
        if self._schema is None:
            raise ValueError("The schema has not been loaded yet. Call `set_schema_parsed` or `set_schema_text` first.")
        if isinstance(self._schema, str):
            self._schema = TypeAdapter(SchemaRootType).validate_json(self._schema)
        return self._schema

    @property
    def object_schema_parsed(self) -> SchemaRootObject:
        """
        Returns the parsed schema and checks if it's a SchemaRootObject.
        Raises a ValueError if the schema has not been loaded yet.
        Automatically parses the schema if `set_schema_text` has been called before.
        """
        if not isinstance(self.schema_parsed, SchemaRootObject):
            raise ValueError("The schema is not an object schema.")
        return self.schema_parsed

    def set_schema_parsed(self, value: SchemaRootType) -> None:
        """Sets the parsed schema."""
        if isinstance(self._schema, str):
            raise ValueError(
                "The schema has already been loaded as a string. If you are sure you want to delete overwrite "
                "the schema text, call `del_schema` first."
            )
        self._schema = value

    @property
    def schema_text(self) -> str:
        """
        Returns the schema as a JSON string.
        Raises a ValueError if the schema has not been loaded yet.
        Always dumps the schema if `get_schema_parsed` has been called before.
        """
        if self._schema is None:
            raise ValueError("The schema has not been loaded yet. Call `set_schema_parsed` or `set_schema_text` first.")
        if isinstance(self._schema, (SchemaRootObject, SchemaRootStrEnum)):
            return self._schema.model_dump_json(indent=2, exclude_unset=True, by_alias=True)
        assert isinstance(self._schema, str)
        return self._schema

    def set_schema_text(self, value: str) -> None:
        """Sets the schema as a JSON string."""
        if isinstance(self._schema, (SchemaRootObject, SchemaRootStrEnum)):
            raise ValueError(
                "The schema has already been parsed. If you are sure you want to delete possible changes "
                "to the parsed schema, call `del_schema` first."
            )
        self._schema = value

    def del_schema(self) -> None:
        """Deletes any schema information (text or json)."""
        self._schema = None

    def __repr__(self) -> str:  # pragma: no cover
        return f"SchemaMeta(name={self.name}, module={self.module}, src={self.src})"

    def __hash__(self) -> int:
        """
        Hashes the schema meta data object.
        This is needed to use the object in a set or as a key in a dictionary.
        """
        return hash((self.name, self.module))

    def __eq__(self, other: object) -> bool:
        """
        Compares the schema meta data object to another object.
        This is needed to use the object in a set or as a key in a dictionary.
        """
        if isinstance(other, SchemaMeta):
            return self.name == other.name and self.module == other.module
        return NotImplemented


T_co = TypeVar("T_co", bound=Hashable, covariant=True)


class Schemas(BaseModel):
    """
    Models a set of schema metadata objects. Most of the set methods are available.
    Also contains the version of the schemas.
    You can retrieve different search indices for the schemas which always reflect the current state of the schemas.
    Even if they were modified externally, the search indices will always be up-to-date.
    The search indices are read-only mappings (views) on the underlying schemas.
    """

    schemas: set[SchemaMeta] = Field(default_factory=set)
    version: Version

    _search_indices: WeakCollection["SearchIndex[Hashable]"] = WeakCollection()
    """
    A collection of weak references to the search indices.
    All created search indices will be saved in this collection as weak reference.
    I.e. if there is no other hard reference to a search index, it will be garbage collected and automatically
    removed from this collection.
    """

    @functools.cached_property
    def names(self) -> "SearchIndex[str]":
        """Returns a search index with the schema names as key."""
        search_index = SearchIndex(self, key_func=lambda schema: schema.name)
        self._search_indices.add(search_index)
        return search_index

    @functools.cached_property
    def modules(self) -> "SearchIndex[tuple[str, ...]]":
        """Returns a search index with the schema modules (as tuple) as key."""
        search_index = SearchIndex(self, key_func=lambda schema: schema.module)
        self._search_indices.add(search_index)
        return search_index

    def _flag_search_indices(self) -> None:
        """
        Flags all search indices to be updated.
        They will be updated automatically on the next access.
        This method will be called whenever schemas are added or removed.
        """
        for index in self._search_indices:
            # pylint: disable=protected-access
            index._schemas_updated = True

    def equals(self, other: "Schemas", equality_type: Literal["meta", "structure"] = "meta") -> bool:
        """
        Check if these schemas are equal to the other schemas.
        The equality type can be either 'meta' or 'structure'.
        'meta' means that the schemas are equal if they have the same metadata (except the source path).
        'structure' means that the schemas are equal if they have the same metadata and the
        same structure (see `schema_parsed`).
        """
        if self.version != other.version:
            return False
        for schema_self, schema_other in zip(
            sorted(self.schemas, key=lambda schema: schema.name), sorted(other.schemas, key=lambda schema: schema.name)
        ):
            if schema_self.name != schema_other.name or schema_self.module != schema_other.module:
                return False
            if equality_type == "structure":
                if schema_self.schema_parsed != schema_other.schema_parsed:
                    return False
        return True

    @staticmethod
    def _get_schemas(schemas_or_set: "Schemas | set[SchemaMeta]") -> set[SchemaMeta]:
        """
        Helper method to get the schemas from a Schemas object or a set of SchemaMeta objects.
        """
        if isinstance(schemas_or_set, Schemas):
            return schemas_or_set.schemas
        if isinstance(schemas_or_set, set):
            return schemas_or_set
        raise TypeError(f"Expected Schemas or set[SchemaMeta], got {type(schemas_or_set)}")

    # ****************** Functions to mimic a set ******************
    def __contains__(self, item: object) -> bool:
        return self.schemas.__contains__(item)

    def __iter__(self) -> Iterator[SchemaMeta]:  # type: ignore[override]
        # BaseModel already defines an __iter__ method but f*ck it, I don't need it.
        return self.schemas.__iter__()

    def __len__(self) -> int:
        return self.schemas.__len__()

    def __le__(self, other: "Schemas | set[SchemaMeta]") -> bool:
        return self.schemas.__le__(self._get_schemas(other))

    def __lt__(self, other: "Schemas | set[SchemaMeta]") -> bool:
        return self.schemas.__lt__(self._get_schemas(other))

    def __eq__(self, other: object) -> bool:
        if isinstance(other, Schemas):
            return self.schemas.__eq__(other.schemas) and self.version == other.version
        return self.schemas.__eq__(other)

    def __ne__(self, other: object) -> bool:
        return not self.__eq__(other)

    def __gt__(self, other: "Schemas | set[SchemaMeta]") -> bool:
        return self.schemas.__gt__(self._get_schemas(other))

    def __ge__(self, other: "Schemas | set[SchemaMeta]") -> bool:
        return self.schemas.__ge__(self._get_schemas(other))

    def __and__(self, other: "Schemas | set[SchemaMeta]") -> set[SchemaMeta]:
        return self.schemas.__and__(self._get_schemas(other))

    def __or__(self, other: "Schemas | set[T_co]") -> set[SchemaMeta | T_co]:
        return self.schemas.__or__(other.schemas if isinstance(other, Schemas) else other)  # type: ignore[operator]
        # No idea why mypy is complaining here

    def __sub__(self, other: "Schemas | set[SchemaMeta]") -> set[SchemaMeta]:
        return self.schemas.__sub__(self._get_schemas(other))

    def __xor__(self, other: "Schemas | set[T_co]") -> set[SchemaMeta | T_co]:
        return self.schemas.__xor__(other.schemas if isinstance(other, Schemas) else other)  # type: ignore[operator]
        # No idea why mypy is complaining here

    def isdisjoint(self, other: Iterable[object]) -> bool:
        """Return True if the set has no elements in common with other.
        Sets are disjoint iff their intersection is the empty set."""
        return self.schemas.isdisjoint(other)

    def add(self, item: SchemaMeta) -> None:
        """Add an element to this set."""
        prev_len = len(self.schemas)  # To prevent double contain check. This should be faster.
        self.schemas.add(item)
        if len(self.schemas) != prev_len:
            self._flag_search_indices()

    def update(self, *items_iters: Iterable[SchemaMeta]) -> None:
        """Update this set with the union of sets as well as any other iterable items."""
        prev_len = len(self.schemas)  # To prevent double contain check. This should be faster.
        self.schemas.update(*items_iters)
        if len(self.schemas) != prev_len:
            self._flag_search_indices()

    def remove(self, item: SchemaMeta) -> None:
        """Remove an element from this set; it must be a member."""
        prev_len = len(self.schemas)  # To prevent double contain check. This should be faster.
        self.schemas.remove(item)
        if len(self.schemas) != prev_len:
            self._flag_search_indices()


class SearchIndex(Generic[T_co]):
    # Note: Mapping is invariant in key-type due to problems with the __getitem__ method.
    # See https://github.com/python/typing/issues/445 and https://github.com/python/typing/pull/273.
    # But in this case it would actually be safe to use a covariant type variable as key type.
    """
    This class is a (read-only) mapping view of an arbitrary key type T to schema metadata objects.
    This view will always reflect the current state of the Schemas collection.
    SearchIndex is covariant in T since it is read-only.
    For more understanding see e.g. https://stackoverflow.com/a/62863366/21303427
    """

    def __init__(self, schemas: Schemas, key_func: Callable[[SchemaMeta], T_co]):
        self._schemas = schemas
        self._schemas_updated = False
        self._key_func = key_func
        self._index: dict[T_co, SchemaMeta]
        self._build_index()

    def _build_index(self) -> None:
        """(Re)build the index from the schemas"""
        self._index = {}
        for schema in self._schemas:
            key = self._key_func(schema)
            if key in self._index:
                raise ValueError(f"Duplicate key: {key}")
            self._index[key] = schema

    def _update_index_if_flagged(self) -> None:
        """Update the index if the schemas were updated"""
        if self._schemas_updated:
            self._build_index()
            self._schemas_updated = False

    # ****************** Functions to mimic a mapping ******************
    def __getitem__(self, item: T_co) -> SchemaMeta:  # type: ignore[misc]
        # Cannot use a covariant type variable as a parameter
        # This is actually not true if the object is read-only.
        # Since there is no way of telling mypy that this object is read-only, I have to ignore this error.
        self._update_index_if_flagged()
        return self._index.__getitem__(item)

    def __iter__(self) -> Iterator[T_co]:
        self._update_index_if_flagged()
        return self._index.__iter__()

    def __len__(self) -> int:
        return len(self._schemas)

    def __contains__(self, other: object) -> bool:
        self._update_index_if_flagged()
        return self._index.__contains__(other)

    def keys(self) -> KeysView[T_co]:
        """Return a view of the keys of the mapping."""
        self._update_index_if_flagged()
        return self._index.keys()

    def items(self) -> ItemsView[T_co, SchemaMeta]:
        """Return a view of the items of the mapping."""
        self._update_index_if_flagged()
        return self._index.items()

    def values(self) -> ValuesView[SchemaMeta]:
        """Return a view of the values of the mapping."""
        return self._schemas  # type: ignore[return-value]

    @overload
    def get(self, key: T_co, /) -> SchemaMeta | None:  # type: ignore[misc]
        ...

    @overload
    def get(self, key: T_co, /, default: SchemaMeta) -> SchemaMeta:  # type: ignore[misc]
        ...

    def get(self, key: T_co, /, default: SchemaMeta | None = None) -> SchemaMeta | None:  # type: ignore[misc]
        """Return the value for key if key is in the dictionary, else default."""
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
