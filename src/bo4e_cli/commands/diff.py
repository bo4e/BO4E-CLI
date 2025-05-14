"""
Contains all commands for comparing JSON-schemas of different BO4E versions.
"""

from enum import StrEnum
from pathlib import Path
from typing import Annotated

import typer

from bo4e_cli.commands.dummy import dummy
from bo4e_cli.diff.diff import diff_schemas as get_changes_by_diff_schemas
from bo4e_cli.io.console import CONSOLE
from bo4e_cli.io.schemas import read_schemas

sub_app_diff = typer.Typer(
    help="Command group for comparing JSON-schemas of different BO4E versions. "
    "See 'diff --help' for more information."
)


class MatrixOutputType(StrEnum):
    """
    A custom type for the diff matrix command.
    """

    JSON = "json"
    CSV = "csv"


@sub_app_diff.command("schemas")
def diff_schemas(
    *,
    input_dir_base: Annotated[Path, typer.Argument(show_default=False)],
    input_dir_comp: Annotated[Path, typer.Argument(show_default=False)],
    output_file: Annotated[
        Path, typer.Option("--output", "-o", help="The JSON-file to save the differences to.", show_default=False)
    ],
) -> None:
    """
    Compare the JSON-schemas in the two input directories and save the differences to the output file (JSON).

    The output file will contain the differences in JSON-format. It will also contain information about the
    compared versions.
    """
    schemas_base = read_schemas(input_dir_base)
    schemas_comp = read_schemas(input_dir_comp)
    with CONSOLE.status("Comparing JSON-schemas...", spinner="squish"):
        changes = get_changes_by_diff_schemas(schemas_base, schemas_comp)
    CONSOLE.print("Compared JSON-schemas.")
    output_file.parent.mkdir(parents=True, exist_ok=True)
    with open(output_file, "w", encoding="utf-8") as file:
        file.write(changes.model_dump_json(indent=2))
    CONSOLE.print("Saved Diff to file:", output_file)


@sub_app_diff.command("matrix")
def diff_matrix(
    *,
    input_diff_files: Annotated[
        list[Path],
        typer.Argument(
            show_default=False,
            help="An unordered list of Diff-files created by the 'diff schemas' command. "
            "At least one file must be provided.\n\n"
            "The versions inside these diff files must be consecutive and ascending. I.e. you have to be able to "
            "create an ascending series of versions from the versions in the diff files. E.g.:\n\n"
            "|      file 3      | -> |      file 1      | -> |      file 2      |\n\n"
            "| v1.0.0 -> v1.0.2 |    | v1.0.2 -> v1.3.0 |    | v1.3.0 -> v2.0.0 |",
        ),
    ],
    output_file: Annotated[
        Path, typer.Option("--output", "-o", help="The file to save the difference matrix to.", show_default=False)
    ],
    output_type: Annotated[
        MatrixOutputType,
        typer.Option(
            "--output-type",
            "-t",
            help="The type of the output file.",
        ),
    ] = MatrixOutputType.JSON,
    emotes: Annotated[
        bool,
        typer.Option(
            help="Whether to use emojis in the output file. "
            "If disabled, text will be used instead to indicate the type of change.",
        ),
    ] = False,
) -> None:
    """
    Create a difference matrix from the diff-files created by the 'diff schemas' command.

    The datastructure models a table where the columns are a list of
    ascending versions where each column is a comparison to the version before. This means that the very first version
    will not appear in the matrix as text.

    The rows will represent each model such that each cell indicates how the model has changed between the two versions.
    """
    dummy(input_diff_files=input_diff_files, output_file=output_file, output_type=output_type, emotes=emotes)


@sub_app_diff.command("bump")
def diff_version_bump_type(
    *,
    diff_file: Annotated[Path, typer.Argument(show_default=False)],
) -> None:
    """
    Determine the release bump type according to a diff file created by 'diff schemas'.
    Prints 'functional' or 'technical' to stdout.

    The version tags inside the diff file are ignored. The bump type will be determined using the list of changes.
    """
    dummy(diff_file=diff_file)
