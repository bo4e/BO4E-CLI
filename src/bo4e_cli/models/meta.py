"""
This module contains the models for the GitHub API queries.
"""

import re
from pathlib import Path
from typing import (
    AbstractSet,
    Annotated,
    Callable,
    Hashable,
    ItemsView,
    Iterable,
    Iterator,
    KeysView,
    Literal,
    Mapping,
    Set,
    TypeVar,
    ValuesView,
)

from pydantic import BaseModel, ConfigDict, Field, HttpUrl, TypeAdapter, computed_field

from bo4e_cli.models.schema import SchemaRootType
from bo4e_cli.models.weakref import WeakCollection

REGEX_VERSION = re.compile(
    r"^v(?P<major>\d{6})\."
    r"(?P<functional>\d+)\."
    r"(?P<technical>\d+)"
    r"(?:-rc(?P<candidate>\d*))?"
    r"(?:\+dev(?P<commit>\w+))?$"
)


class Version(BaseModel):
    """
    A version of the BO4E-Schemas.
    """

    major: int
    functional: int
    technical: int
    candidate: int | None
    commit: str | None
    """
    The commit hash.
    When retrieving the version from a commit which has no tag on it, the version will have the commit hash
    after the last version tag in the history.
    """

    @classmethod
    def from_str(cls, version: str) -> "Version":
        """
        Parse a version string into a Version object e.g. 'v202401.0.1-rc8+dev12asdf34' or 'v202401.0.1'.
        Raises a ValueError if the version string is invalid.
        """
        match = REGEX_VERSION.match(version)
        if match is None:
            raise ValueError(f"Invalid version: {version}")
        return cls(**match.groupdict())

    def is_release_candidate(self) -> bool:
        """Check if the version is a release candidate."""
        return self.candidate is not None

    def is_local_commit(self) -> bool:
        """Check if the version is on a commit without a tag."""
        return self.commit is not None

    def __str__(self) -> str:
        version = f"v{self.major}.{self.functional}.{self.technical}"
        if self.candidate is not None:
            version += f"-rc{self.candidate}"
        if self.commit is not None:
            version += f"+dev{self.commit}"
        return version

    def __eq__(self, other: object) -> bool:
        if isinstance(other, Version):
            return super().__eq__(other)
        if isinstance(other, str):
            return str(self) == other
        return NotImplemented

    def __ne__(self, other: object) -> bool:
        return not self.__eq__(other)


class SchemaMeta(BaseModel):
    """
    This class represents a schema meta data object.
    """

    model_config = ConfigDict(frozen=True)

    name: str
    """ E.g. 'Marktlokation' """
    module: tuple[str, ...]
    """ E.g. ('bo', 'Marktlokation') or ('ZusatzAttribut',) """
    src: HttpUrl | Path
    """ Either an online URL or a local file path """

    _schema: SchemaRootType | str | None = None

    @computed_field
    @property
    def relative_path(self) -> Path:
        """E.g. 'bo/Marktlokation.json' or 'ZusatzAttribut.json'"""
        return Path(*self.module).with_suffix(".json")

    @property
    def src_url(self) -> HttpUrl:
        """Returns the source as an online URL. Raises a ValueError if the source is not a URL."""
        if isinstance(self.src, Path):
            raise ValueError("The source is not an online URL.")
        return self.src

    @property
    def src_path(self) -> Path:
        """Returns the source as a local file path. Raises a ValueError if the source is not a path."""
        if not isinstance(self.src, Path):
            raise ValueError("The source is not a local file path.")
        return self.src

    def get_schema_parsed(self) -> SchemaRootType:
        """
        Returns the parsed schema.
        Raises a ValueError if the schema has not been loaded yet.
        Automatically parses the schema if `set_schema_text` has been called before.
        """
        if self._schema is None:
            raise ValueError("The schema has not been loaded yet. Set `schema_parsed` or `schema_text` first.")
        if isinstance(self._schema, str):
            self._schema = TypeAdapter(SchemaRootType).validate_json(self._schema)
        return self._schema

    def get_schema_text(self) -> str:
        """
        Returns the schema as a JSON string.
        Raises a ValueError if the schema has not been loaded yet.
        Always dumps the schema if `get_schema_parsed` has been called before.
        """
        if self._schema is None:
            raise ValueError("The schema has not been loaded yet. Call `set_schema_parsed` or `set_schema_text` first.")
        if isinstance(self._schema, SchemaRootType):
            return self._schema.model_dump_json(indent=2, exclude_unset=True, by_alias=True)
        return self._schema

    def set_schema_text(self, value: str) -> None:
        """Sets the schema as a JSON string."""
        if isinstance(self._schema, SchemaRootType):
            raise ValueError(
                "The schema has already been parsed. If you are sure you want to delete possible changes "
                "to the parsed schema, call `del_schema_parsed` first."
            )
        self._schema = value

    def __repr__(self) -> str:  # pragma: no cover
        return f"SchemaMeta(name={self.name}, module={self.module}, src={self.src})"


T = TypeVar("T", bound=Hashable, covariant=True)


class Schemas(BaseModel):
    """
    Models a set of schema metadata objects. Most of the set methods are available.
    Also contains the version of the schemas.
    You can retrieve different search indices for the schemas which always reflect the current state of the schemas.
    Even if they were modified externally, the search indices will always be up-to-date.
    The search indices are read-only mappings (views) on the underlying schemas.
    """

    schemas: Annotated[set[SchemaMeta], Field(default_factory=set)]
    version: Version

    _search_indices: WeakCollection["SearchIndex[Hashable]"] = WeakCollection()
    """
    A collection of weak references to the search indices.
    All created search indices will be saved in this collection as weak reference.
    I.e. if there is no other hard reference to a search index, it will be garbage collected and automatically
    removed from this collection.
    """

    @property
    def search_index_by_cls_name(self) -> "SearchIndex[str]":
        """Returns a search index with the schema names as key."""
        search_index = SearchIndex(self, key_func=lambda schema: schema.name)
        self._search_indices.add(search_index)
        return search_index

    @property
    def search_index_by_module(self) -> "SearchIndex[tuple[str, ...]]":
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
            index._schemas_updated = True

    def equals(self, other: "Schemas", equality_type: Literal["meta", "structure"] = "meta") -> bool:
        """
        Check if these schemas are equal to the other schemas.
        The equality type can be either 'meta' or 'structure'.
        'meta' means that the schemas are equal if they have the same metadata (except the source path).
        'structure' means that the schemas are equal if they have the same metadata and the
        same structure (see `get_schema_parsed()`).
        """
        if self.version != other.version:
            return False
        for schema_self, schema_other in zip(
            sorted(self.schemas, key=lambda schema: schema.name), sorted(other.schemas, key=lambda schema: schema.name)
        ):
            if schema_self.name != schema_other.name or schema_self.module != schema_other.module:
                return False
            if equality_type == "structure":
                if schema_self.get_schema_parsed() != schema_other.get_schema_parsed():
                    return False
        return True

    # ****************** Functions to mimic a set ******************
    def __contains__(self, item: object) -> bool:
        return self.schemas.__contains__(item)

    def __iter__(self) -> Iterator[SchemaMeta]:
        return self.schemas.__iter__()

    def __len__(self) -> int:
        return self.schemas.__len__()

    def __le__(self, other: AbstractSet[object]) -> bool:
        return self.schemas.__le__(other)

    def __lt__(self, other: AbstractSet[object]) -> bool:
        return self.schemas.__lt__(other)

    def __eq__(self, other: object) -> bool:
        if isinstance(other, Schemas):
            return self.schemas.__eq__(other.schemas) and self.version == other.version
        return self.schemas.__eq__(other)

    def __ne__(self, other: object) -> bool:
        return not self.__eq__(other)

    def __gt__(self, other: AbstractSet[object]) -> bool:
        return self.schemas.__gt__(other)

    def __ge__(self, other: AbstractSet[object]) -> bool:
        return self.schemas.__ge__(other)

    def __and__(self, other: AbstractSet[object]) -> Set[SchemaMeta]:
        return self.schemas.__and__(other)

    def __or__(self, other: AbstractSet[T]) -> Set[SchemaMeta | T]:
        return self.schemas.__or__(other)

    def __sub__(self, other: AbstractSet[SchemaMeta | None]) -> Set[SchemaMeta]:
        return self.schemas.__sub__(other)

    def __xor__(self, other: AbstractSet[T]) -> Set[SchemaMeta | T]:
        return self.schemas.__xor__(other)

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


class SearchIndex(Mapping[T, SchemaMeta]):
    """
    This class is a (read-only) mapping view of an arbitrary key type T to schema metadata objects.
    This view will always reflect the current state of the Schemas collection.
    SearchIndex is covariant in T since it is read-only.
    For more understanding see e.g. https://stackoverflow.com/a/62863366/21303427
    """

    def __init__(self, schemas: Schemas, key_func: Callable[[SchemaMeta], T]):
        self._schemas = schemas
        self._schemas_updated = False
        self._key_func = key_func
        self._index: dict[T, SchemaMeta]
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
        """Return a view of the keys of the mapping."""
        self._update_index_if_flagged()
        return self._index.keys()

    def items(self) -> ItemsView[T, SchemaMeta]:
        """Return a view of the items of the mapping."""
        self._update_index_if_flagged()
        return self._index.items()

    def values(self) -> ValuesView[SchemaMeta]:
        """Return a view of the values of the mapping."""
        return self._schemas

    def get(self, key: T, default: SchemaMeta | None = None) -> SchemaMeta | None:
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
