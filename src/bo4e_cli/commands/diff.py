"""
Contains all commands for comparing JSON-schemas of different BO4E versions.
"""

from enum import StrEnum
from pathlib import Path
from typing import Annotated

import typer
from more_itertools import one

from bo4e_cli.commands.parser import set_quiet_mode
from bo4e_cli.diff.diff import diff_schemas as get_changes_by_diff_schemas
from bo4e_cli.diff.matrix import create_compatibility_matrix, create_graph_from_changes, get_path_through_di_path_graph
from bo4e_cli.diff.version import check_version_bump
from bo4e_cli.io.changes import read_changes_from_diff_files, write_changes
from bo4e_cli.io.console import CONSOLE
from bo4e_cli.io.matrix import write_compatibility_matrix_csv, write_compatibility_matrix_json
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
    write_changes(changes, output_file)
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
            "--emotes",
            "-e",
            help="Whether to use emojis in the output file. "
            "If disabled, text will be used instead to indicate the type of change.",
        ),
    ] = False,
) -> None:
    """
    Create a difference matrix from the diff-files created by the 'diff schemas' command.

    The data structure models a table where the columns are a list of
    ascending versions where each column is a comparison to the version before. This means that the very first version
    will not appear in the matrix as text.

    The rows will represent each model such that each cell indicates how the model has changed between the two versions.
    """
    with CONSOLE.status("Reading changes from diff files...", spinner="squish"):
        graph = create_graph_from_changes(read_changes_from_diff_files(*input_diff_files))
        path = get_path_through_di_path_graph(graph)
    CONSOLE.print("Read changes from diff files.")
    with CONSOLE.status("Creating compatibility matrix...", spinner="squish"):
        compatibility_matrix = create_compatibility_matrix(graph, path, use_emotes=emotes)
    CONSOLE.print("Created compatibility matrix.")

    with CONSOLE.status(f"Saving compatibility matrix to file {output_file} ...", spinner="squish"):
        if output_type == MatrixOutputType.JSON:
            write_compatibility_matrix_json(output_file, compatibility_matrix)
        elif output_type == MatrixOutputType.CSV:
            write_compatibility_matrix_csv(output_file, compatibility_matrix, path)
        else:
            raise ValueError(f"Unknown output type: {output_type}.")
    CONSOLE.print(f"Saved compatibility matrix to file {output_file}.")


@sub_app_diff.command("version-bump")
def diff_version_bump_type(
    *,
    diff_file: Annotated[Path, typer.Argument(show_default=False)],
    allow_major_bump: Annotated[
        bool, typer.Option("--allow-major-bump", "-a", help="Allow major version bumps.")
    ] = False,
    quiet: Annotated[
        bool,
        typer.Option(
            "--quiet",
            "-q",
            help="Suppress output messages. Can't be set together with verbose option. "
            "If set, the program will exit with code 0 and no output if the version bump is valid, "
            "or with code 1 and an error message if it is invalid. "
            "If not set, the program will print a message to stdout indicating whether the version bump "
            "is valid or not and will always exit with code 0.",
        ),
    ] = False,
) -> None:
    """
    Determine the release bump type according to a diff file created by 'diff schemas'.
    Prints 'valid' to stdout if the version bump is valid.
    Otherwise, a descriptive error message is printed (to stdout).

    The bump type will be determined using the list of changes and compared to the corresponding versions inside the
    diff file.
    """
    set_quiet_mode(quiet)
    changes = one(read_changes_from_diff_files(diff_file))
    try:
        check_version_bump(changes, major_bump_allowed=allow_major_bump)
        CONSOLE.print("The version bump is valid.")
    except ValueError as error:
        if quiet:
            print(f"Invalid version bump: {error}")
            raise typer.Exit(code=1) from error
        CONSOLE.print(f"Invalid version bump: {error}", style="warning")
