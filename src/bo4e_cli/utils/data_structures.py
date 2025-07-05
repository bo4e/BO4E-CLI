"""
Contains custom data structures that are used in the CLI.
"""

from collections.abc import Hashable
from typing import Generic, ItemsView, Iterator, KeysView, Mapping, TypeVar, ValuesView

from pydantic import RootModel

K = TypeVar("K", bound=Hashable)
V = TypeVar("V")


class RootModelDict(Mapping[K, V], RootModel[dict[K, V]], Generic[K, V]):
    """
    This pydantic RootModel is a dict-like object and implements the corresponding methods.
    """

    root: dict[K, V]

    def __getitem__(self, k: K) -> V:
        return self.root[k]

    def __setitem__(self, k: K, v: V) -> None:
        self.root[k] = v

    def __iter__(self) -> Iterator[K]:  # type: ignore[override]
        # Don't care about the implementation of the BaseModel iterator
        return iter(self.root)

    def __len__(self) -> int:
        return len(self.root)

    def __contains__(self, k: object) -> bool:
        return k in self.root

    def items(self) -> ItemsView[K, V]:
        """Return a new view of the dictionary's items (key, value).

        Returns:
            ItemsView: A view of the dictionary's items.
        """
        return self.root.items()

    def keys(self) -> KeysView[K]:
        return self.root.keys()

    def values(self) -> ValuesView[V]:
        return self.root.values()
