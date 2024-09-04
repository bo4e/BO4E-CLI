"""
Contains utility functions for working with iterators
"""

from itertools import cycle
from typing import Any, Generator, Iterable


def zip_cycle(
    *iterables: Iterable[Any], els_to_cycle: Iterable[Any] = tuple()
) -> Generator[tuple[Any, ...], None, None]:
    """
    Zip an arbitrary number of iterables together (just like builtin zip) and add the elements from ``els_to_cycle``
    to the end of each tuple.
    These elements are cycled through, i.e. in each tuple they will have the same value.
    """
    els_cycle_iterators = [cycle([el]) for el in els_to_cycle]
    for els in zip(*iterables, *els_cycle_iterators):
        yield els
