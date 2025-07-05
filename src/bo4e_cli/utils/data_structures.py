"""
Contains custom data structures that are used in the CLI.
"""

from collections.abc import Hashable
from typing import Any, Generic, ItemsView, Iterator, KeysView, Mapping, NamedTuple, TypeVar, ValuesView

from more_itertools import one
from pydantic import RootModel

from bo4e_cli.utils.iterator import side_effect

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


class PathGraphNode(NamedTuple, Generic[K]):
    """
    Represents a node in a path graph.
    Each node has a value and a list of neighbors.
    """

    key: K
    info: dict[str, Any]
    from_key: "K | None" = None
    to_key: "K | None" = None


class PathGraphChecker(Generic[K]):
    def __init__(self):
        self._nodes: dict[K, PathGraphNode[K]] = {}
        self._iter_lock = False

    def _start_iter(self) -> None:
        """
        Marks the graph as being iterated over.
        This is used to prevent modifications while iterating.
        """
        self._iter_lock = True

    def _end_iter(self) -> None:
        """
        Marks the graph as no longer being iterated over.
        This is used to allow modifications after iteration.
        """
        self._iter_lock = False

    def _check_not_iterating(self) -> None:
        """
        Checks if the graph is currently being iterated over.

        :raises RuntimeError: If the graph is currently being iterated over.
        """
        if self._iter_lock:
            raise RuntimeError("Cannot modify the path graph while it is being iterated over.")

    def add_node(self, node: PathGraphNode[K]) -> None:
        """
        Adds a node to the path graph.

        :param node: The node to add.
        :raises ValueError: If a node with the same key already exists in the path graph.
        :raises RuntimeError: If the graph is currently being iterated over.
        """
        self._check_not_iterating()
        if node.key in self._nodes:
            raise ValueError(f"Node with key {node.key} already exists in the path graph.")
        self._nodes[node.key] = node

    def add_edge(
        self,
        from_key: K,
        to_key: K,
        *,
        auto_create_missing_nodes: bool = False,
        from_info: dict[str, Any] | None = None,
        to_info: dict[str, Any] | None = None,
    ) -> None:
        """
        Adds an edge between two nodes in the path graph.

        :param from_key: The key of the node from which the edge starts.
        :param to_key: The key of the node to which the edge points.
        :param auto_create_missing_nodes: If `True`, automatically creates nodes if they do not exist in the path graph.
        :param from_info: Optional additional information for the "from" node.
        :param to_info: Optional additional information for the "to" node.
        :raises ValueError: If either node does not exist in the path graph and `auto_create_missing_nodes` is `False`,
            or if the edge already exists.
        :raises RuntimeError: If the graph is currently being iterated over.
        """
        self._check_not_iterating()
        if not auto_create_missing_nodes and (from_key not in self._nodes or to_key not in self._nodes):
            raise ValueError(
                "Both nodes must exist in the path graph if `auto_create_missing_nodes` is not set to `True`."
            )

        if from_key not in self._nodes:
            self._nodes[from_key] = PathGraphNode(key=from_key, info=from_info or {})
        elif self._nodes[from_key].to_key is not None:
            raise ValueError(f"Node {from_key} already has an outgoing edge to {self._nodes[from_key].to_key}.")
        elif from_info is not None and self._nodes[from_key].info != from_info:
            raise ValueError(f"Node {from_key} already exists with different info: {self._nodes[from_key].info}")

        if to_key not in self._nodes:
            self._nodes[to_key] = PathGraphNode(key=to_key, info=to_info or {})
        elif self._nodes[to_key].from_key is not None:
            raise ValueError(f"Node {to_key} already has an incoming edge from {self._nodes[to_key].from_key}.")
        elif to_info is not None and self._nodes[to_key].info != to_info:
            raise ValueError(f"Node {to_key} already exists with different info: {self._nodes[to_key].info}")

        self._nodes[from_key] = self._nodes[from_key]._replace(to_key=to_key)
        self._nodes[to_key] = self._nodes[to_key]._replace(from_key=from_key)

    def get_starting_nodes(self) -> Iterator[PathGraphNode[K]]:
        """
        Returns an iterator over the nodes that have no incoming edges.

        :return: An iterator over the starting nodes.
        """
        return side_effect(
            None,
            (node for node in self._nodes.values() if node.from_key is None),
            before=self._start_iter,
            after=self._end_iter,
        )

    def get_ending_nodes(self) -> Iterator[PathGraphNode[K]]:
        """
        Returns an iterator over the nodes that have no outgoing edges.

        :return: An iterator over the ending nodes.
        """
        return side_effect(
            None,
            (node for node in self._nodes.values() if node.to_key is None),
            before=self._start_iter,
            after=self._end_iter,
        )

    def check_graph(self) -> None:
        """
        Checks the path graph for consistency.

        :raises ValueError: If the graph is not a connected path graph.
        """
        starting_nodes = list(self.get_starting_nodes())
        if len(starting_nodes) != 1:
            raise ValueError(f"Path graph must have exactly one starting node, found: {len(starting_nodes)}")
        ending_nodes = list(self.get_ending_nodes())
        if len(ending_nodes) != 1:
            raise ValueError(f"Path graph must have exactly one ending node, found: {len(ending_nodes)}")

    def __iter__(self) -> Iterator[PathGraphNode[K]]:
        """
        Returns an iterator over the nodes in the path graph.

        :return: An iterator over the nodes.
        :raises ValueError: If the graph is not a connected path graph.
        """
        self.check_graph()
        self._start_iter()

        def iter_inner() -> Iterator[PathGraphNode[K]]:
            starting_node = one(self.get_starting_nodes())
            yield starting_node
            current_node = self._nodes[starting_node]
            while current_node.to_key is not None:
                current_node = self._nodes[current_node.to_key]
                yield current_node
            self._end_iter()

        return iter_inner()
