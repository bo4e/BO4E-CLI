"""
Contains utility functions for working with iterators
"""

from itertools import cycle
from typing import Any, Callable, Generator, Hashable, Iterable

from mypyc.ir.ops import TypeVar


def zip_cycle(
    *iterables: Iterable[Any], els_to_cycle: Iterable[Any] = tuple()
) -> Generator[tuple[Any, ...], None, None]:
    """
    Zip an arbitrary number of iterables together (just like builtin zip) and add the elements from ``els_to_cycle``
    to the end of each tuple.
    These elements are cycled through, i.e. in each tuple they will have the same value.
    """
    yield from zip(*iterables, *(cycle([el]) for el in els_to_cycle))


K = TypeVar("K", bound=Hashable)
V = TypeVar("V")


def sorted_dict_items(dictionary: dict[K, V], key_func: Callable[[K, V], int]) -> Generator[tuple[K, V], None, None]:
    """
    Return a generator that yields the items of the dictionary sorted by their keys
    """
    yield from sorted(dictionary.items())
