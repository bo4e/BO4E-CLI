"""
Contains all commands for comparing JSON-schemas of different BO4E versions.
"""

from enum import StrEnum
from pathlib import Path
from typing import Annotated

import typer

sub_app_diff = typer.Typer(
    help="Command group for comparing JSON-schemas of different [#8cc04d]BO[/][#617d8b]4E[/] versions. "
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
):
    """
    Compare the JSON-schemas in the two input directories and save the differences to the output file (JSON).

    The output file will contain the differences in JSON-format. It will also contain information about the
    compared versions.
    """
    pass


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
):
    """
    Create a difference matrix from the diff-files created by the 'diff schemas' command.

    The datastructure models a table where the columns are a list of
    ascending versions where each column is a comparison to the version before. This means that the very first version
    will not appear in the matrix as text.

    The rows will represent each model such that each cell indicates how the model has changed between the two versions.
    """
    pass


@sub_app_diff.command("bump")
def diff_version_bump_type(
    *,
    diff_file: Annotated[Path, typer.Argument(show_default=False)],
):
    """
    Determine the release bump type according to a diff file created by 'diff schemas'.
    Prints 'functional' or 'technical' to stdout.

    The version tags inside the diff file are ignored. The bump type will be determined using the list of changes.
    """
    pass
