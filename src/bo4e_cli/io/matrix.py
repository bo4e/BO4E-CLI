"""
Contains io functions for the compatibility matrix of BO4E versions.
"""

import csv
from pathlib import Path
from typing import Sequence

from bo4e_cli.models.matrix import CompatibilityMatrix


def write_compatibility_matrix_csv(
    output: Path, compatibility_matrix: CompatibilityMatrix, versions: Sequence[str]
) -> None:
    """
    Create a compatibility matrix csv file from the given compatibility matrix.
    """
    right_arrow = "\u21a6"
    output.parent.mkdir(parents=True, exist_ok=True)
    with open(output, "w", encoding="utf-8") as file:
        csv_writer = csv.writer(file, delimiter=",", lineterminator="\n", escapechar="/")
        header = (
            "",
            f"{versions[0]} {right_arrow} {versions[1]}",
            *(f"{right_arrow} {version}" for version in versions[2:]),
        )
        csv_writer.writerow(header)

        for module, entries in compatibility_matrix.items():
            row = (module, *(entry.compatibility.value for entry in entries))
            csv_writer.writerow(row)


def write_compatibility_matrix_json(output: Path, compatibility_matrix: CompatibilityMatrix) -> None:
    """
    Create a compatibility matrix json file from the given compatibility matrix.
    """
    output.parent.mkdir(parents=True, exist_ok=True)
    with open(output, "w", encoding="utf-8") as file:
        file.write(compatibility_matrix.model_dump_json(indent=2))
