# type: ignore
"""
This module provides functions to compare the BO4E JSON schemas of different versions.
It also contains functions to query GitHub for the latest BO4E versions to compare with the schemas of the current
work tree.
Additionally, it implements a little cache functionality to avoid multiple downloads of the same versions e.g.
if you're testing locally.
"""

# pylint: skip-file
import logging
from pathlib import Path

from bo4e_cli.io.git import get_last_n_tags
from bo4e_cli.io.matrix import write_compatibility_matrix_csv

logger = logging.getLogger(__name__)


def create_tables_for_doc(
    compatibility_matrix_output_file: Path,
    gh_version: str,
    *,
    gh_token: str | None = None,
    last_n_versions: int = 2,
) -> None:
    """
    Creates the compatibility matrix for the documentation. The output is a csv file. This can be referenced
    inside Sphinx documentation. See https://sublime-and-sphinx-guide.readthedocs.io/en/latest/tables.html#csv-files
    for more information.
    If you have problems with rate limiting, please set gh_token.
    The compatibility matrix will be built for last_n_versions + the current version in the checkout working directory.
    If you set last_n_versions = 0 all versions since v202401.0.0 will be compared.
    Note: The matrix will never contain the first version as column. Each column is a comparison to the version before.
    Note: Only functional releases will be compared since technical releases are enforced to be fully compatible.
    See https://github.com/bo4e/BO4E-python/issues/784
    """
    logger.info("Retrieving the last %d release versions", last_n_versions)
    versions = list(reversed(list(get_last_n_tags(last_n_versions, ref=gh_version, exclude_technical_bumps=True))))
    logger.info("Comparing versions iteratively: %s", " -> ".join([*versions, gh_version]))
    changes_iterables = compare_bo4e_versions_iteratively(versions, gh_version, gh_token=gh_token)
    logger.info("Building namespaces")
    changes = {key: list(value) for key, value in changes_iterables.items()}
    namespaces = {version: list(get_namespace(BO4E_BASE_DIR / version)) for version in versions}
    namespaces[gh_version] = list(get_namespace(BO4E_BASE_DIR / gh_version))
    logger.info("Creating compatibility matrix")
    write_compatibility_matrix_csv(compatibility_matrix_output_file, [*versions, gh_version], namespaces, changes)


def test_create_tables_for_doc() -> None:
    """
    Test the create_tables_for_doc function locally without building the entire documentation.
    Needs the JSON schemas to be present in /json_schemas with TARGET_VERSION set to "local".
    """
    create_tables_for_doc(
        Path(__file__).parents[1] / "compatibility_matrix.csv",
        "local",
        last_n_versions=0,
    )
