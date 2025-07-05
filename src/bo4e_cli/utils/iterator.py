"""
Contains utility functions for working with iterators
"""

from itertools import cycle
from typing import Any, Callable, Generator, Iterable, Iterator, TypeVar, overload

from more_itertools import chunked

T = TypeVar("T")


def zip_cycle(
    *iterables: Iterable[Any], els_to_cycle: Iterable[Any] = tuple()
) -> Generator[tuple[Any, ...], None, None]:
    """
    Zip an arbitrary number of iterables together (just like builtin zip) and add the elements from ``els_to_cycle``
    to the end of each tuple.
    These elements are cycled through, i.e. in each tuple they will have the same value.
    """
    yield from zip(*iterables, *(cycle([el]) for el in els_to_cycle))


@overload
def side_effect(
    func: Callable[[T], Any] | None,
    iterable: Iterable[T],
    chunk_size: None = ...,
    before: Callable[[], Any] | None = ...,
    after: Callable[[], Any] | None = ...,
) -> Iterator[T]: ...


@overload
def side_effect(
    func: Callable[[list[T]], Any] | None,
    iterable: Iterable[T],
    chunk_size: int,
    before: Callable[[], Any] | None = ...,
    after: Callable[[], Any] | None = ...,
) -> Iterator[T]: ...


def side_effect(
    func: Callable[[T], Any] | Callable[[list[T]], Any] | None,
    iterable: Iterable[T],
    chunk_size: int | None = None,
    before: Callable[[], Any] | None = None,
    after: Callable[[], Any] | None = None,
) -> Iterator[T]:
    """
    Invoke *func* on each item in *iterable* (or on each *chunk_size* group
    of items) before yielding the item.

    `func` is an optional function that takes a single argument. Its return value
    will be discarded. If no `func` is provided, the items will simply be yielded.

    *before* and *after* are optional functions that take no arguments. They
    will be executed before iteration starts and after it ends, respectively.

    `side_effect` can be used for logging, updating progress bars, or anything
    that is not functionally "pure."

    Emitting a status message:

        >>> from more_itertools import consume
        >>> func = lambda item: print('Received {}'.format(item))
        >>> consume(side_effect(func, range(2)))
        Received 0
        Received 1

    Operating on chunks of items:

        >>> pair_sums = []
        >>> func = lambda chunk: pair_sums.append(sum(chunk))
        >>> list(side_effect(func, [0, 1, 2, 3, 4, 5], 2))
        [0, 1, 2, 3, 4, 5]
        >>> list(pair_sums)
        [1, 5, 9]

    Writing to a file-like object:

        >>> from io import StringIO
        >>> from more_itertools import consume
        >>> f = StringIO()
        >>> func = lambda x: print(x, file=f)
        >>> before = lambda: print(u'HEADER', file=f)
        >>> after = f.close
        >>> it = [u'a', u'b', u'c']
        >>> consume(side_effect(func, it, before=before, after=after))
        >>> f.closed
        True

    Copied from `more-itertools <https://more-itertools.readthedocs.io/en/stable/api.html#more_itertools.side_effect>`_.
    """
    try:
        if before is not None:
            before()

        if func is None:
            yield from iterable
            return

        if chunk_size is None:
            for item in iterable:
                func(item)  # type: ignore[arg-type]
                yield item
        else:
            for chunk in chunked(iterable, chunk_size):
                func(chunk)  # type: ignore[arg-type]
                yield from chunk
    finally:
        if after is not None:
            after()
