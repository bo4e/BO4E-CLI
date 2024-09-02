"""
This module provides a collection that holds weak references to its elements.
"""

import weakref
from typing import Collection, Iterator, TypeVar

T = TypeVar("T")  # invariant because collection is mutable


class WeakCollection(Collection[T]):
    """
    A mutable collection that holds weak references to its elements.
    I.e. if there is no other hard reference to an element, it will be garbage collected and automatically
    removed from this collection.
    """

    def __init__(self, init_collection: Collection[T] | None = None):
        self._elements: list[weakref.ReferenceType[T]] = []
        if init_collection is not None:
            for item in init_collection:
                self.add(item)

    def __contains__(self, item: object) -> bool:
        return any(ref() == item for ref in self._elements)

    def __iter__(self) -> Iterator[T]:
        for ref in self._elements:
            item = ref()
            if item is None:
                raise RuntimeError("Weak reference is dead but was not removed from this collection.")
            yield item

    def __len__(self) -> int:
        return len(self._elements)

    def add(self, item: T) -> None:
        """
        Add an item to the collection.
        If the item has no hard references anywhere it will get garbage collected and removed from this collection.
        """
        self._elements.append(weakref.ref(item, self._remove_weakref))

    def remove(self, item: T) -> None:
        """
        Remove an item from the collection.
        """
        self._elements.remove(weakref.ref(item))

    def _remove_weakref(self, item: weakref.ReferenceType[T]) -> None:
        self._elements.remove(item)
