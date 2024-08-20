"""
This module contains the generate command.
"""

from enum import StrEnum
from pathlib import Path
from typing import Annotated

import typer

from bo4e_cli.commands.dummy import dummy
from bo4e_cli.commands.entry import app


class GenerateType(StrEnum):
    """
    A custom type for the generate command.
    """

    PYTHON_PYDANTIC_V1 = "python-pydantic-v1"
    PYTHON_PYDANTIC_V2 = "python-pydantic-v2"
    PYTHON_SQL_MODEL = "python-sql-model"


@app.command()
def generate(
    *,
    input_dir: Annotated[
        Path, typer.Option("--input", "-i", help="The directory to read the JSON-schemas from.", show_default=False)
    ],
    output_dir: Annotated[
        Path, typer.Option("--output", "-o", help="The directory to save the generated code to.", show_default=False)
    ],
    output_type: Annotated[
        GenerateType, typer.Option("--output-type", "-t", help="The type of code to generate.", show_default=False)
    ],
    clear_output: Annotated[
        bool, typer.Option(help="Clear the output directory before saving the generated code.")
    ] = True,
):
    """
    Generate the [#8cc04d]BO[/][#617d8b]4E[/] models from the JSON-schemas in the input directory and save them in the
    output directory.

    Several output types are available, see --output-type.
    """
    dummy(input_dir=input_dir, output_dir=output_dir, output_type=output_type, clear_output=clear_output)
