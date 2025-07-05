"""
This module contains the logic to create the compatibility matrix from a list of changes.
"""

import itertools
from typing import Any, Iterator, Sequence, cast

import networkx as nx

from bo4e_cli.diff.filters import is_change_critical
from bo4e_cli.models.changes import Change, Changes, ChangeSymbol, ChangeText, ChangeType
from bo4e_cli.models.matrix import CompatibilityMatrix, CompatibilityMatrixEntry
from bo4e_cli.models.meta import Schemas

# def iter_sorted_through_list_of_changes(changes_list: Iterator[Changes]) -> Iterator[Changes]:
#     """
#     Iterate through a list of Changes objects, sorted by their old_version.
#     Before starting the iteration, the function ensures that the `old_version` and `new_version` of each Changes object
#     form a single path graph (note that a `Changes` object acts as an edge of the graph).
#
#     :param changes_list: An iterator of Changes objects. The iterator will be exhausted.
#     :return: An iterator over Changes objects, sorted by their old_version.
#     :raises ValueError: If the changes do not form a single valid path graph.
#     """
#     path_graph = PathGraphChecker[str]()
#     changes_index = {}
#     for changes in changes_list:
#         changes_index[str(changes.old_version)] = changes
#         path_graph.add_edge(
#             str(changes.old_version),
#             str(changes.new_version),
#             auto_create_missing_nodes=True,
#             from_info={"schemas": changes.old_schemas},
#             to_info={"schemas": changes.new_schemas},
#         )
#         # Note: We won't need the "schemas" field in further code but the add_edge function automatically checks
#         # that the old and new schemas across different Changes objects are equal if the version tag is the same.
#
#     for node in islice(path_graph, 0, -1):
#         yield changes_index[node.key]


def _check_node(graph: nx.DiGraph, node_key: str, **node_attrs: Any) -> None:
    """
    Check if a node exists in the graph and has no incoming edges.
    Raises an error if the node does not exist or has incoming edges.
    """
    if node_key not in graph:
        graph.add_node(node_key, **node_attrs)
        return
    if graph.nodes[node_key] != node_attrs:
        raise ValueError(f"Node {node_key} already exists with different attributes: {graph.nodes[node_key]}")


def create_graph_from_changes(changes_list: Iterator[Changes]) -> nx.DiGraph:
    """
    Create a directed graph from a list of Changes objects.

    :param changes_list: An iterable of Changes objects.
    :return: A directed graph where each Changes object is represented as an edge.
    """
    graph = nx.DiGraph()
    for changes in changes_list:
        _check_node(graph, str(changes.old_version), schemas=changes.old_schemas)
        _check_node(graph, str(changes.new_version), schemas=changes.new_schemas)
        graph.add_edge(str(changes.old_version), str(changes.new_version), changes=changes)
    return graph


def get_path_through_di_path_graph(graph: nx.DiGraph) -> list[str]:
    """
    Check if the given graph is a valid path graph, meaning it has exactly one starting node and one ending node,
    and all nodes in between have exactly one incoming and one outgoing edge.

    :param graph: The directed graph to check.
    :raises ValueError: If the graph is not a valid path graph.
    """
    start_key: str | None = None
    end_key: str | None = None
    for node_key in graph.nodes:
        in_degree = graph.in_degree(node_key)
        out_degree = graph.out_degree(node_key)

        if in_degree == 0:
            if start_key is not None:
                raise ValueError(f"Graph has more than one starting node: {start_key} and {node_key}")
            start_key = node_key
        elif out_degree == 0:
            if end_key is not None:
                raise ValueError(f"Graph has more than one ending node: {end_key} and {node_key}")
            end_key = node_key
        elif in_degree != 1 or out_degree != 1:
            raise ValueError(f"Node {node_key} must have exactly one incoming and one outgoing edge.")
    if start_key is None or end_key is None:
        raise ValueError("Graph must have exactly one starting and one ending node.")

    return nx.shortest_path(graph, start_key, end_key)
    # Note: The shortest_path function has big performance impact since each node has at most one outgoing edge.


def determine_symbol(
    changes: Sequence[Change], schemas: Schemas, cls: tuple[str, ...], *, use_emotes: bool = False
) -> ChangeSymbol | ChangeText:
    """
    Determine the symbol of a change.
    """
    symbol_model = ChangeSymbol if use_emotes else ChangeText
    if len(changes) == 1 and changes[0].type == ChangeType.CLASS_REMOVED:
        return symbol_model.REMOVED
    if len(changes) == 1 and changes[0].type == ChangeType.CLASS_ADDED:
        return symbol_model.ADDED
    if cls not in schemas.modules:
        return symbol_model.NON_EXISTENT
    if len(changes) == 0:
        return symbol_model.CHANGE_NONE

    assert all(
        change.type not in (ChangeType.CLASS_ADDED, ChangeType.CLASS_REMOVED) for change in changes
    ), "Internal error: CLASS_ADDED and CLASS_REMOVED must be the only change per class if present."
    if any(is_change_critical(change) for change in changes):
        return symbol_model.CHANGE_CRITICAL
    return symbol_model.CHANGE_NON_CRITICAL


def create_compatibility_matrix(
    path_graph: nx.DiGraph, path: Sequence[str], *, use_emotes: bool = False
) -> CompatibilityMatrix:
    """
    Create a compatibility matrix from the given changes.
    """
    matrix = CompatibilityMatrix()
    all_classes: set[tuple[str, ...]] = set(
        schema.module for _, schemas in path_graph.nodes(data="schemas") for schema in cast(Schemas, schemas)
    )

    for module in sorted(all_classes, key=lambda cls: tuple(cls_part.lower() for cls_part in cls)):
        entries = []
        class_path_str = "/" + "/".join(module) + "#"
        for version_old, version_new in itertools.pairwise(path):
            changes_related_to_class = [
                change
                for change in cast(Changes, path_graph[version_old][version_new]["changes"]).changes
                if change.old_trace.startswith(class_path_str) or change.new_trace.startswith(class_path_str)
            ]
            entries.append(
                CompatibilityMatrixEntry(
                    previous_version=path_graph.nodes[version_old]["schemas"].version,
                    next_version=path_graph.nodes[version_new]["schemas"].version,
                    compatibility=determine_symbol(
                        changes_related_to_class,
                        path_graph.nodes[version_new]["schemas"],
                        module,
                        use_emotes=use_emotes,
                    ),
                )
            )
        matrix[".".join(module)] = entries
    return matrix


# def create_compatibility_matrix_csv(
#     output: Path, path_graph: nx.DiGraph, path: Sequence[str], *, use_emotes: bool = False
# ) -> None:
#     """
#     Create a compatibility matrix csv file from the given changes.
#     """
#     right_arrow = "\u21a6"
#     output.parent.mkdir(parents=True, exist_ok=True)
#     with open(output, "w", encoding="utf-8") as file:
#         csv_writer = csv.writer(file, delimiter=",", lineterminator="\n", escapechar="/")
#         csv_writer.writerow(
#             ("", f"{path[0]} {right_arrow} {path[1]}", *(f"{right_arrow} {version}" for version in path[2:]))
#         )
#         all_classes: set[tuple[str, ...]] = set(
#             schema.module for _, schemas in path_graph.nodes(data="schemas") for schema in cast(Schemas, schemas)
#         )
#
#         for module in sorted(all_classes, key=lambda cls: tuple(cls_part.lower() for cls_part in cls)):
#             row = [module[-1]]
#             class_path_str = "/" + "/".join(module) + "#"
#             for version_old, version_new in itertools.pairwise(path):
#                 changes_related_to_class = [
#                     change
#                     for change in cast(Changes, path_graph[version_old][version_new]["changes"]).changes
#                     if change.old_trace.startswith(class_path_str) or change.new_trace.startswith(class_path_str)
#                 ]
#                 row.append(
#                     determine_symbol(
#                         changes_related_to_class,
#                         path_graph.nodes[version_new]["schemas"],
#                         module,
#                         use_emotes=use_emotes,
#                     ).value
#                 )
#             csv_writer.writerow(row)
