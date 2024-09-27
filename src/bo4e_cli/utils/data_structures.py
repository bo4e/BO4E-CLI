from typing import Generic, Hashable, ItemsView, Iterator, KeysView, Mapping, TypeVar, ValuesView

from pydantic import RootModel

K = TypeVar("K", bound=Hashable)
V = TypeVar("V")


class RootModelDict(Mapping[K, V], RootModel[dict[K, V]], Generic[K, V]):
    root: dict[K, V]

    def __getitem__(self, k: K) -> V:
        return self.root[k]

    def __setitem__(self, k: K, v: V) -> None:
        self.root[k] = v

    def __iter__(self) -> Iterator[K]:
        return iter(self.root)

    def __len__(self) -> int:
        return len(self.root)

    def __contains__(self, k: object) -> bool:
        return k in self.root

    def items(self) -> ItemsView[K, V]:
        return self.root.items()

    def keys(self) -> KeysView[K]:
        return self.root.keys()

    def values(self) -> ValuesView[V]:
        return self.root.values()
