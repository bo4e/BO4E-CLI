import weakref
from typing import Collection, Iterator, TypeVar

T = TypeVar("T")  # invariant because collection is mutable


class WeakCollection(Collection[T]):
    def __init__(self, init_collection: Collection[T] | None = None):
        self._elements: list[weakref.ReferenceType[T]] = []
        if init_collection is not None:
            for item in init_collection:
                self.add(item)

    def __contains__(self, item: object) -> bool:
        return any(ref() == item for ref in self._elements)

    def __iter__(self) -> Iterator[T]:
        return (ref() for ref in self._elements)

    def __len__(self) -> int:
        return len(self._elements)

    def add(self, item: T) -> None:
        self._elements.append(weakref.ref(item, self._remove_weakref))

    def remove(self, item: T) -> None:
        self._elements.remove(weakref.ref(item))

    def _remove_weakref(self, item: weakref.ReferenceType[T]) -> None:
        self._elements.remove(item)
